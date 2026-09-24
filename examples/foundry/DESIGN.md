# Foundry: game design

A living document: what Foundry is meant to be, and why.
It describes the full ten-deck game; what the code does today is marked **exists**, and everything else is **planned**.
Engine work a mechanic calls for is noted under it as **Engine pressure** and tracked in `docs/PLAN.md` and `docs/TODO.md`, never here.
Modelled on the design document in [Roguelike Tutorial in Rust, chapter 44](https://bfnightly.bracketproductions.com/chapter_44.html).

## Characters

The player is one resistance commando, unnamed, with no class and no background to pick.
Every run starts the same: the clothes they came in, a shoulder lamp switched on, and nothing else (**exists**).
Not even a knife.
The first rooms of deck 1 are fought with bare hands and whatever is lying on the floor, so the deck's real question is what the commando picks up first and how long they are willing to wait for something better.
What the commando becomes is found on the decks and chosen at the charges, so the build is the run's story rather than a menu choice.

Nobody on the decks is a friend.
Four sides share the foundry, and each is hostile to the commando and to the others:

- **The droids**, the foundry's own, built on its lines and still at work.
- **The salvage crew**, scavengers who cut their way in to strip the place.
- **Bounty hunters**, who know there is a price on the commando's head.
- **Critters**, the things that live in a derelict plant's leaks, cables and vents.

## Story

The occupation's army marches on droids, and most of them come from one place: a foundry sunk ten decks into the ground, long past its prime and still running.
Nobody maintains it any more, and nobody needs to; the lines keep turning out chassis as long as the reactors feed them.
The resistance sends one commando in with charges.

The brief is short.
Go down, set a charge on the reactor that feeds each section of the plant, set the last one on the core, and come back out the way you came in.
The charges are set to go when the commando is clear, so there is no countdown, only the climb.

The commando is not the only one inside.
A salvage crew has breached the upper decks to strip what the droids are not using, and word of the commando has reached the people who collect bounties.

The story is told in one place only: a line in the log as each deck is entered (**exists** for decks 1-3).
There are no cutscenes and no lore terminals, and nobody on the decks narrates themselves.

## Theme

Gritty sci-fi in a dilapidated industrial plant.
The foundry is not collapsing around the commando; it is simply old, neglected and dangerous: leaking coolant, live cables hanging loose, sections that fell in years ago, fires nobody put out.
It is dark, and getting darker the deeper it goes.
The tone is tense rather than heroic: one person with a lamp, in a building that is still working, surrounded by things that are not.

## Progression

Ten decks in four zones.
Each zone has its own tiles, hazards, light and population, and bands overlap at the edges so difficulty ramps rather than steps.

| Decks | Zone | Light | Hazards | Who lives there |
| --- | --- | --- | --- | --- |
| 1-3 | Assembly | Dim on deck 1, dark below | Coolant spills, coolant vents | Line droids, troopers, probes, coolant rats, scrap crabs |
| 4-6 | Fabrication | Dark, furnace glow | Live cables, fires, fuel vents | Heavy droids, repair drones, salvager camps |
| 7-9 | Reactor ring | Dark | Coolant vents, collapsed sections | Heavier droids, bounty hunters, lamp moths |
| 10 | The core | Dark | All of the above | The foundry's best, and whoever followed you |

All ten decks **exist** and are played; the four zones are **planned**, so every deck is still built as an assembly hall and they differ only in population and depth.

A charge is set on the reactor console on decks 3, 6 and 9, and on the core on deck 10.
Setting a charge takes three whole turns beside the console and opens a pick of one upgrade from three (**exists** on decks 3, 6, 9 and 10).

After the core charge, the commando climbs all ten decks back to the lift they came down on.
The way up is the harder half:

- Every deck is repopulated, drawn from the band of the deepest deck reached rather than the deck's own, so an assembly deck on the way up holds reactor-ring droids.
- Groups are bigger, and the population cap is the deepest deck's.
- Bounty hunters can appear on any deck.
- The alarm does not carry over: each deck starts quiet.

There are no experience levels.
Power comes from gear and from the four upgrade picks, so killing a droid is worth what it drops and nothing more, and slipping past it is always a legitimate choice.

## Gameplay

Turn-based, on a grid, in the classic roguelike shape: every action costs time, measured in hundredths of a step, and everything else on the deck acts on the same clock (**exists**).

A deck is played in three moves: find the way down (or up), find what is worth taking, and decide what to fight.
The dark is the centre of it.
With the lamp lit the commando sees about six tiles into the dark and every droid with a line to them sees them back; with it off, a droid without radar has to be touching the commando to know they are there, and so does the commando (**exists**).
Noise is the other half: a shot or a blow carries ten steps, a probe's alarm fills the deck, and what hears either comes to look (**exists**).

A deck is never static.
New arrivals come in waves from where they would really come from, critters drift to what they feed on, droids get rebuilt, and salvagers strip what the fighting leaves behind (see [Mechanics](#mechanics)).
Waiting is never free and never a flood.

## Goals

- **Primary:** set the four charges and climb back out alive; reaching the lift on deck 1 after the core charge wins the run.
- **Intermediate:** find each deck's lift down, then each deck's lift up; reach each reactor console.
- **Secondary:** equipment worth carrying, the upgrade picks, and surviving the hunters for their weapons.

There is no score yet; the run summary says how far the run got (see [Losing](#losing)).

## Mechanics

### Weapons that run hot or run dry (exists)

An energy weapon carries heat.
Each shot adds heat, and at full heat it locks and stays locked until it has vented all the way back to zero, which it only does on turns it did not fire.
A slug weapon draws a round from the commando's bag per shot, and is dry when the bag is empty.
A locked or dry weapon simply has nothing to shoot with, so a pair of blasters worn in both hands alternates on its own as one locks.

### The ranged ramp (exists for decks 1-3)

A gun is the strongest thing in the game, and in the dark it is stronger still.
The lamp shows six tiles, so a weapon that reaches six kills whatever the commando can see before it arrives, and a weapon that reaches further kills what it cannot see at all.
Both halves of that, the commando's guns and the enemies', are handed out a deck at a time rather than assumed from the first turn.

| Deck | What shoots at the commando | What the commando can find |
| --- | --- | --- |
| 1 | Nothing. The line droid strikes with its clamp; the probe never attacks at all | The monoblade, and the slug pistol on about two decks in five |
| 2 | The trooper droid, at four tiles, inside the lamp's six so there is a turn's warning | Pistols reliably, the ion pistol among them, and the monoblade |
| 3 | The heavy droid, at six tiles, which is exactly as far as the commando can see | Carbines and the mono-axe |
| 4-6 | Cones and fire at close range: welder droids, salvager arc throwers | Rifles, scatterguns, the arc thrower |
| 7-9 | The tracker's rifle, which outranges the lamp: the first thing that shoots out of dark it can see and the commando cannot | The heavy repeater and the ion lance |

Decks 1-3 of that table **exist**: the line droid has no gun, the trooper droid shoots from deck 2 at four tiles, the guns' spawn bands start where the table says, and on deck 1 the only one findable is the slug pistol, which at its weight lies on about two decks in five.
Of the finds further down, the slug rifle (decks 4 on) and the heavy repeater (decks 7 on) **exist**; the rest are **planned**, and so is every shooter below deck 3.

The line droid keeps the foundry's lines running and fights with the clamp it does that with, which suits a worker droid better than the bolt it used to carry.
What shoots on the way down is what the foundry was built to make: the trooper chassis, walking off the end of the line it was assembled on.
So the first gun pointed at the commando is a product, and taking it is the commando arming themselves from the occupation's own stock.
The slug pistol is deck 1's exception for the same reason it is a fair one: it is a gun on a leash, useless the turn the bag runs out.

On the climb back out every deck draws from the deepest band reached, so the ramp only runs one way; a deck 1 revisited after the core holds trackers.

### Light and stealth (exists, extended)

The shoulder lamp, droid radar, the rangefinder helmet's dark sight and the notice rules **exist**.
**Planned:** more light in the world, so the dark has shape.

- Ceiling lamps and sparking panels, which can be shot out to darken a room, shake off lamp moths, or open a way past.
- Furnace glow in fabrication and fire glow wherever something burns, which cannot be put out by shooting.

**Engine pressure:** a shot today lands on whoever stands under the cursor.
Shooting out a light needs a shot that can land on a prop or a tile, and a light that goes out when the thing carrying it breaks.

### The alarm (exists)

A probe that notices the commando keeps three to five tiles off and sounds a klaxon on every turn it knows where they are.
Whoever hears it comes to where the probe stood, and still has to notice the commando once there.

### Hazards (planned)

Scenery and static hazards, placed when a deck is built; nothing gets worse over time.

- **Coolant spills:** cryo underfoot; they feed coolant rats.
- **Live cables:** electricity on the tile and beside it, to anything standing there. A hidden cable that shocks whoever steps on it **exists**, as a prop from deck 2 down; the arc beside it is **planned**.
- **Collapsed sections:** rubble that blocks movement and sight, cutting a deck's rooms into a different shape.
- **Fires:** burning wreckage and fuel, using the engine's `FirePlugin`, which the incendiary grenade already uses.
- **Vents:** some vent gas on a cycle of their own, using the engine's `GasPlugin` and `Vents`. Coolant vapour (assembly, reactor ring) is cold and thick enough to hide in, and chills whoever breathes it. Fuel vapour (fabrication) burns, and sits beside furnaces and fires. The commando cannot pass through a vent.

### Reinforcements (planned)

A deck gains a new group every 75 to 125 turns, rolled from the run's seed.
A group is one row of a banded, weighted spawn table, in the shape fantasy-rogue uses: either one kind with a range of copies, or a composed group of kinds, each with its own count.
It arrives awake, out of the commando's sight and a fair distance off.
The deck's live population has a cap that grows with depth, and a clock that fires over the cap re-rolls and skips.

Where a group comes in is decided by its faction, not by a second table:

- **Droids** come off a fabricator's assembly line.
- **Critters** come through a vent.
- **Salvagers** come through a breach, on decks with a camp.
- **Bounty hunters** step off a lift, as if they had followed the commando.

A row with no way onto the current deck is rolled again.

**Engine pressure:** fantasy-rogue's horde clock lives in the game.
The engine should own the loop: a spawning plugin that decides when and where a group appears (out of sight, at a distance, from its own stream, under a cap), with the game supplying the table and the entry points.

### The assembly line (planned)

A fabricator sits at the head of a conveyor.
A droid group leaves it one chassis at a time: the chassis moves down the belt over a few turns, its glyph changing as it is built, and activates at the end.
A group of three is a queue the commando can see coming.
A chassis on the belt can be shot, and the next one keeps coming; while the line is busy the deck's clock rolls no other droid group.
Fabricators cannot be destroyed, which is why the rates are low.

### Breaches and salvager camps (planned)

Salvagers live in camps: a section of a deck they cut their way into, entered by a breach fixed when the deck is built.
Droids patrol the rest of the deck, so the two sides meet at the edges, or where the commando's noise draws them together.
Some decks in fabrication are held by the salvagers outright, with few droids on them.
Which decks have camps, and which are held, varies with the seed.

### Remains (planned)

What dies leaves remains: a droid wreck, a salvager's body, a crushed critter.
Remains are the pivot of the living map.

- **Repair drones** walk to a droid wreck and rebuild it after a few turns, so the medic is the first thing to kill.
- **Salvagers** strip droid wrecks, carrying off what they pull out of them, which drops again when they die.
- **Scrap crabs** race everyone to wrecks and dropped items, and carry them off.
- **The commando** can search a droid wreck, a turn or two for slugs, a cell or a heat sink cartridge.

**Engine pressure:** remains belong in the engine, as an opt-in `RemainsPlugin`.
The engine leaves remains on a death, recording what died and when, shows them to minds in the snapshot so a tactic can walk to them the way `Scavenge` walks to gear, and lets them burn.
What remains mean (rebuilt, stripped, carried off, searched) stays the game's.

### Critters that feed (planned)

- **Coolant rats** go to coolant spills and drink them dry, so a spill shrinks and rats cluster at leaks.
- **Lamp moths** swarm toward light, the commando's lamp included, which puts a price on keeping it lit and a use on shooting out a panel.
- **Scrap crabs** go for loot and wrecks (see [Remains](#remains-planned)).

**Engine pressure:** a mind that goes to a spill or to light needs to see either in its snapshot; a game can push a `Sense` of its own, and if two games do, it belongs in the engine.

## Items

No identification: sci-fi gear comes with a spec sheet.
No weight: the pack has a fixed number of slots, and ammunition stacks.
No currency: nothing is bought or sold.

Damage types are **kinetic**, **energy**, **ion**, **electricity**, **cryo** and **thermal**, and each hazard deals one of them, so a resist is also a way across the map.
All but cryo **exist**, and the damage table they are read through is played: a chassis takes half of kinetic and double of ion, and flesh a quarter less of energy and a quarter of ion.

Ion and electricity are separate, and they are not the same idea:

- **Ion** is the anti-machine type. It jams a droid's radar as it does today (**exists**), and it tells against a chassis, but it does little to flesh, so an ion weapon is dead weight against salvagers, hunters and critters.
- **Electricity** is raw current: live cables, a capacitor let go, a salvager's cable gun. It hurts whatever it touches, chassis or flesh, plate conducts it rather than stopping it, and it is what most of the foundry's own wiring deals.

That split is what makes gear choice a real one.
A commando carrying only ion walks the droid decks safely and meets the salvage crew with nothing.

### Weapons

| Name | Type | Kind | Runs on | Status |
| --- | --- | --- | --- | --- |
| Monoblade | Melee, throwable | Kinetic | - | exists |
| Mono-axe | Melee, two hands | Kinetic | - | exists |
| Hand blaster | Pistol | Energy | Heat | exists |
| Blaster carbine | Rifle, two hands | Energy | Heat | exists |
| Ion pistol | Pistol, jams radar | Ion | Heat | exists |
| Slug pistol | Pistol | Kinetic | Slugs | exists |
| Slug rifle | Rifle, two hands, reaches past the lamp | Kinetic | Slugs | exists |
| Scattergun | Cone | Kinetic | Slugs | planned |
| Heavy repeater | Rifle, two hands, two shots a step | Energy | Heat | exists |
| Ion lance | Beam, jams radar | Ion | Heat | planned |
| Arc thrower | Cone, salvager-made | Electricity | Fuel cells | planned |
| Coolant sprayer | Cone | Cryo | Coolant cells | planned |
| Cutting torch | Melee | Thermal | Fuel cells | planned |

### Armor

| Name | Slot | Does | Status |
| --- | --- | --- | --- |
| Commando helmet | Head | A point of armor | exists |
| Rangefinder helmet | Head | A point of armor, and six tiles of dark sight | exists |
| Scrap plate | Torso | A point of armor | exists |
| Composite plate | Torso | Two points, and it resists energy | exists |
| Combat gauntlets | Arms | A point of armor | exists |
| Armored greaves | Legs | A point of armor | exists |
| Insulated suit | Torso | Resists electricity, so live cables and a salvager's arc thrower both tell less | planned |
| Cryo liner | Torso | Resists cryo, for the coolant decks | planned |
| Thermal cloak | Torso | Resists thermal, for fabrication's fires and welders | planned |
| Shielded plate | Torso | Resists ion, and worth taking only off a deck the droids hold | planned |
| Hunter's plate | Torso | The best armor in the game; dropped by bounty hunters and nowhere else | planned |

A resist is a zone choice rather than something to stack: nearly all of them sit in the torso slot, so a commando wears the suit the deck calls for and decides whether the others are worth the pack slots.
Shielded plate makes the point sharpest, since ion is what the droids deal and nothing else does.

### Implants (planned)

Worn in an implant slot, each granting an ability or a sense.

- **Optic implant:** dark sight, without the helmet.
- **Arc capacitor:** grants arc discharge, an electricity burst.
- **Reflex implant:** grants dash.
- **Dermal plating:** armor that takes no other slot.

### Tools and oddments (planned)

Everything that is not worn or wielded, and the reason the pack's slots are worth arguing over.
Most of it is salvager kit, found in a camp or taken off the crew; the rest comes out of droid wrecks, which give up slugs, a cell or a cartridge to a commando willing to spend a turn or two searching one.
None of it is identified and none of it is bought: a spec sheet comes with the thing.

| Name | Kind | Does | Status |
| --- | --- | --- | --- |
| Slug | Ammunition | One round for a slug weapon; stacks | exists |
| Coolant cell | Ammunition | Charges for the coolant sprayer; stacks | planned |
| Fuel cell | Ammunition | Charges for the arc thrower and the cutting torch; stacks | planned |
| Heat sink cartridge | Ammunition | Spent on a locked energy weapon, venting it to zero in one turn instead of five | planned |
| Medkit | Medical | Twenty points of mending, two a turn over ten turns, which is ten turns of the deck to live through first; a second kit starts the ten again rather than mending twice as fast | exists |
| Stim | Medical | Seven to thirteen, closed at once, for the commando who did not pick the Stims upgrade | exists |
| Burn dressing | Medical | Puts out a burning commando, and heals a little of what the fire took | planned |
| Thermal wrap | Medical | Clears the chill coolant vapour leaves, and holds off the next one for a while | planned |
| Frag grenade | Grenade | A kinetic burst; the one that works on everything and excels at nothing | exists |
| Incendiary grenade | Grenade | Thermal, and it leaves what it lands on burning | exists |
| Cryo grenade | Grenade | Cryo, and it chills whatever is caught in it | planned |
| Ion grenade | Grenade | Jams every radar in the burst, so a pack of droids goes blind at once | exists |
| Arc grenade | Grenade | Electricity, and it shorts what it lands on: a droid stalls, a panel goes dark | planned |
| Smoke grenade | Grenade | A gas cloud that hides whatever stands in it, the commando included | exists |
| Flare | Light | Thrown; lights a room for a while, and pulls lamp moths to it instead of to the lamp | planned |
| Chem light | Light | Dropped; a dim landmark that lasts the deck, draws nothing and lights nothing worth seeing by | planned |
| Noisemaker | Noise | Thrown; makes a shot's worth of noise where it lands, and what searches goes there rather than here | planned |
| Breaching charge | Access | Blows a hole through a wall or a collapsed section, and the whole deck hears it | planned |
| Pry bar | Access | Opens a jammed bulkhead, and serves as a poor melee weapon in a pinch | planned |
| Door clamp | Access | Holds a door shut, so what wants through it has to break it first | planned |
| Cable spool | Access | Rigged at a collapsed shaft: down to the deck below without finding its lift | planned |
| Signal jammer | Anti-droid | Switched on for a few turns; every droid radar within eight tiles is blind while it runs | planned |
| Radar decoy | Anti-droid | Thrown; radar reads it as the commando until a droid reaches it and finds a box | planned |
| Survey slate | Knowledge | A salvager's, dropped: shows the deck's shape and where its lifts are, never what walks it | planned |

Four of them are worth calling out as the ones the design leans on.
The heat sink cartridge is what makes a single energy weapon viable at all, since the alternative is five quiet turns.
The noisemaker and the radar decoy are the Ghost's answer to a deck that hunts by sound and by radar, and they are the reason the build is not only about the lamp.
The cable spool and the breaching charge exist so that a deck's shape is negotiable: a route down that the map did not offer, bought with noise.

A grenade is a throwable item with a land trigger, a burst where it comes to rest, and one charge that the landing spends (**exists**).
So a burst, a fire and a gas already land on a tile, and the cryo grenade is content once cryo and a chill status are.
Every grenade is thrown from the pack: `t` opens it on the first thing that can be thrown, and `t` again throws the row picked out, a grenade or a blade alike.

**Engine pressure:** what is still missing is a thrown thing that stays where it lands and goes on doing something: a light that belongs to an item and keeps burning where it was dropped, a noise made by a thing rather than by an action, since a noisemaker is loud and the commando throwing it is not, and a decoy a radar reads as an actor.
The cable spool and the breaching charge want the last one, a warp or a dug tile the game asks for at a place of its choosing.

## Challenge

Difficulty comes from depth, from the climb, and from the deck around the commando, never from a timer.

- **Depth:** each zone brings harder kinds and darker decks; spawn bands overlap so the ramp is smooth.
- **Group size:** a kind's groups grow with depth (**exists** for line droids).
- **The climb:** revisited decks at the deepest band reached, bigger groups, hunters anywhere.
- **Reinforcements:** a deck left alone gains a group every 75 to 125 turns, up to a cap.
- **No regeneration:** healing is medkits and stims only, so every fight costs something that is not given back.
- **Interaction:** a deck's sides work against each other as well as against the commando, which a careful player turns to their advantage and a careless one gets caught in.

## Builds

There are no classes; a build is what the run found and picked.
The design supports a few on purpose, and every item and upgrade should serve at least one of them.
More will come as melee, ranged and damage-type content is built out over time.

- **Ghost:** lamp off, melee, an ion pistol to blind the radar that would see through the dark.
- **Gunner:** a pair of blasters alternating as each runs hot, armor, and the upgrades that make heat cheaper.
- **Demolitionist:** grenades, fire and gas as weapons, using the deck's own hazards.
- **Bruiser:** heavy melee and heavy armor, resists matched to the zone.

### Upgrades

One pick of three at each charge, four in a run, kept for the run.

| Name | Does | Serves | Status |
| --- | --- | --- | --- |
| Stims | Grants stims: heals, twenty-turn cooldown | Everyone | exists |
| Uplink | One more tile of range on every ranged weapon | Gunner | exists |
| Servos | A tenth faster at everything | Everyone | exists |
| Heat sinks | Weapons vent faster | Gunner | planned |
| Dampers | Strikes and shots make less noise | Ghost | planned |
| Shaded lamp | The lamp lights less but still shows the way | Ghost | planned |
| Blast packing | Grenades land wider | Demolitionist | planned |
| Hazard seals | Resist cryo, electricity and thermal a little | Demolitionist, Bruiser | planned |
| Hydraulics | Melee hits harder | Bruiser, Ghost | planned |

## Losing

Permadeath: a dead commando ends the run, and nothing carries over to the next.
The run ends on a summary on the engine's ending screen (**exists** in its general form, and draws the outcome, the seed and the turn itself; Foundry pushes none of its own sections onto `EndingView` yet): the deepest deck reached, the charges set, the killer, and the build.
A run is deterministic from its seed, so a summary can be shared and the run replayed.

## Art

ASCII only, on a 100 by 40 grid, with colour doing the work (**exists**).

- **Colour by faction:** droids in steel grey and tan, salvagers in rust orange, bounty hunters in a hard white on red, critters muted.
- **Letter case by weight:** `d` a line droid, `D` a heavy droid; lowercase is light, uppercase is heavy.
- **Hazards in their damage type's colour:** electricity blue-white, ion pale blue, cryo pale cyan, thermal orange-red.
- **Light and gas:** memory and darkness shading, flickering flames, and gas drawn as a haze where it hides what is behind it, all from the engine.

## Technical

Rust, on Bevy, on `rl-engine`: this crate is a worked example of the engine and depends on nothing a game could not.
It uses the engine's turn clock, minds, combat, lighting, noise, stealth, abilities, quests, loot tables, panels, remains, fire and gas.
Content is RON in `assets/`, each file headed by the full option space.
What a thing is and where it turns up are separate files: `monsters.ron` and `items.ron` say what each kind is, and `monster_spawns.ron` and `item_spawns.ron` say on which decks, how often, and in what numbers, a row per band.
`\` opens a cheat menu for testing a deck without playing the ones above it: reveal the map, heal, godmode, the lift down or up, and a search that puts any item in the pack.
The screen is cut once in `src/main.rs`: the map, a log along the bottom, and a rail of vitals, gear and what is nearby.

## Enemies

A monster's kind is content in `assets/monsters.ron`; its faction, wits, sight, hearing and spawn band are all data.

### Droids

| Kind | Glyph | Role | Zone | Status |
| --- | --- | --- | --- | --- |
| Line droid | `d` | Clamp arm, melee only, groups grow with depth | Assembly, fabrication | exists |
| Probe droid | `p` | Radar, keeps its distance and sounds the alarm | Assembly, fabrication | exists |
| Trooper droid | `t` | A finished chassis off the line: shoots at four tiles, drops its blaster | Assembly from deck 2, fabrication | exists |
| Heavy droid | `D` | Armored, hits hard, slow, shoots at six | Assembly from deck 3, fabrication, reactor ring | exists |
| Repair drone | `u` | Rebuilds droid wrecks | Fabrication onward | planned |
| Welder droid | `w` | Thermal melee, sets fires | Fabrication | planned |
| Coolant droid | `k` | Cryo cone, walks through vapour | Reactor ring | planned |
| Warden | `W` | Heavy, electricity, radar | Reactor ring, core | planned |

### Salvagers

| Kind | Glyph | Role | Status |
| --- | --- | --- | --- |
| Scrapper | `s` | Melee, strips wrecks | planned |
| Cutter | `s` | Cutting torch, thermal | planned |
| Salvage gunner | `s` | Slug weapons | planned |
| Crew boss | `S` | Tougher, better gear | planned |

Salvagers are sapient: they open doors, pick up, equip, throw and flee.

### Bounty hunters

| Kind | Glyph | Role | Status |
| --- | --- | --- | --- |
| Tracker | `h` | Long-range energy rifle, radar | planned |
| Enforcer | `H` | Heavy weapons, hunter's plate | planned |

Hunters are just stronger kinds: sapient, carrying the best weapons in the game, and dropping them.
They appear from the reactor ring down, and anywhere on the climb.

### Critters

| Kind | Glyph | Role | Status |
| --- | --- | --- | --- |
| Coolant rat | `r` | Drinks spills, flees when hurt | exists, feeding planned |
| Scrap crab | `c` | Carries off loot and parts | exists, carrying planned |
| Lamp moth | `m` | Swarms toward light | exists, swarming planned |

## Abilities

An ability comes from one of three places: an upgrade, an implant, or a consumable.
Abilities run on cooldowns only; there is no energy pool, since heat and ammunition are already the resources the commando manages.

| Ability | From | Does | Status |
| --- | --- | --- | --- |
| Stims | Stims upgrade | Heals, twenty-turn cooldown | exists |
| Field stims | Stim | Closes a wound at once, and the shot is spent | exists |
| Med gel | Medkit | Two a turn for ten turns, and the kit is spent | exists |
| Arc discharge | Arc capacitor | Electricity burst on everything adjacent, chassis or flesh | planned |
| Dash | Reflex implant | Several tiles in a straight line, one turn | planned |
| Overclock | Upgrade | A few shots that add no heat | planned |
| Grenades | Consumable | Thrown, by type (see [Items](#tools-and-oddments-planned)) | frag, incendiary, ion and smoke exist |

## Other ideas

Undecided design, not tasks.

- Destroying a fabricator, silencing its line for good.
- Sealing a vent, or throwing gas into one.
- Cable mites that nest along live cables and arc when disturbed.
- Arc fire that shorts a live cable for a while, opening a path.
- Line droids hauling along conveyor routes until alarmed, so a quiet deck shows a routine.
- Salvagers and hunters that talk through the log: a line when they spot the commando, or when one of them falls.
  It would make them read as people rather than as stat blocks, but a log that narrates itself is a log nobody reads, so the budget and the triggers want deciding before a single line is written.
- Starting kits to choose from.
- A score.

### Props

Things for the decks beyond the crate, the locker, the console and the live cable, ordered by how much they need that does not exist yet, least first.

- **Fuel drum:** breaks into a thermal burst and sets what is around it burning; data alone today.
- **Crushed conveyor:** derailed track laid in lines across a bay, blocking movement but not sight, so it is a lane to shoot across and not walk across, with half-built chassis on it to search as wreckage.
- **Parts bin:** a searchable pile of severed optics and limbs named after the deck's own droids, so it says who lives here before they are met, and scrap crabs forage from it.
- **Coolant drum:** breaks into coolant vapour that hides and chills whoever is in it; waits on cryo and a chill status.
- **Coolant line:** overhead piping over a placed spill that feeds coolant rats, and breaking it floods the corridor with vapour; waits on cryo.
- **Control terminal:** wired to one machine nearby, and a hack of a few turns reprograms it, darkens the room or overloads a dock, while a glitched one fails and sounds a klaxon; no text to read.
- **Charging dock:** a faulty wall alcove where the commando can vent an energy weapon to zero heat for a jolt of electricity, a hurt droid goes to recharge, and breaking it arcs into whatever is docked; waits on minds using a prop's offers.
- **Mag plate:** a striped floor plate that holds anything metal stepping on it, droids always and the commando only in plate, and hacked it pulls every metal thing within two tiles onto it once; waits on a status that refuses a step but not an action, and a trigger that picks whom it lands on.
- **Cutting rig:** a ceiling laser whose red wire marks a line across a room, firing thermal along the whole line at whatever crosses it, then going dark for a few turns to recharge; waits on a line-shaped trigger area and a light carried by a prop.
- **Assembly arm:** a malfunctioning manipulator that sweeps a telegraphed strip of cells on a cycle, crushing and shoving droid and commando alike, and hacked strikes anything but the commando; waits on a strip-shaped area, a cycle moment and a trigger that picks whom it lands on.
- **Furnace and slag chute:** the furnace blocks and glows, the chute destroys outright whatever is shoved into it, and the scrap heap beside it gives slugs or cells to a loud search; waits on forced movement counting as entering a cell, a commando with a shove, and noise made by a thing.
- **Hanging load:** a half-built mech on an overhead hoist that drops when the hoist is broken or hacked, crushing a small burst, leaving rubble and waking the deck; waits on an effect that changes a tile.
