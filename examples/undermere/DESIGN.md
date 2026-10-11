# Undermere: game design

What the game is, for whoever builds the next part of it.
It is a real game rather than a worked example: when it needs something the engine lacks, the engine grows it, as it did for Foundry.
The parts are built in order, and each part's spec under `docs/superpowers/specs/` says how; this page says what and why, and is kept current as parts land.

## The pitch

A lakeside town, Mereholt, has grown up around the Drowned Gate: a ring of standing stones on the shore that opens onto the Undermere, the ruins of a kingdom that sank under the lake an age ago.
Ten floors down, under the halls, the caves, the crypts, the flooded cathedral and the forge, sits the thing that drowned it.
The adventurer walks out of the town with what they can carry and whoever they can pay, goes through the gate, and either kills what waits at the bottom or joins the dead down there.

A traditional roguelike: turn-based, grid-based, permadeath, one life per run, drawn in glyphs.
The model is Brogue: a dungeon that is beautiful to look at, items as the only progression, mechanics that can be read off the screen, and choices that cost something.

## Pillars

1. **Light is the art.** Every floor is lit by what is in it: the adventurer's lantern, braziers, glowing fungus, lava, magic. The palette changes with the depth, and colour and shadow carry the mood that a tileset would elsewhere.
2. **Monsters are characters.** Every kind has a tell, something seen before it is understood, and a trick. Depth stacks tricks: a rat has one, a necromancer three, the thing at the bottom many.
3. **Items are the only progression, and every gain costs something.** No experience and no levels. Power comes from what is found and where scarce enchantment is spent, and every strong thing has a price: a slot, a drawback, gold, noise, a curse.
4. **Readable, with secrets.** The examine panel says what is known in plain words. What is not yet known stays unknown until it is used or seen: an unidentified potion, a monster's trick it has not shown yet.
5. **No two runs show everything.** Biomes, vaults, mini-bosses, the final boss, shop stock and mercenaries are all drawn from pools larger than one run can see.

## A run

1. The title menu: Continue, New Game, Records, Settings, Exit.
2. Mereholt, at dusk. Shops, townsfolk, the tavern's mercenaries, the gate.
3. Floors one to ten, each built the first time it is reached and kept whole after.
4. The run is won the moment the boss on floor ten dies, and lost when the adventurer dies.
5. The end screen gives the score, line by line, and the run's story in numbers; the run joins the records.

The adventurer can come back to town mid-run: by climbing out, or by a waystone, ten turns of kneeling that any harm breaks off, and the gate then leads back down to where they knelt, so a trip home buys time at the shops and never moves them forward.

## The screen

Square map tiles, each two text cells wide with its glyph centred, so light pools, blasts and sight are round, and text keeps a terminal's proportions.
The grid is 160 by 45 cells: the map on the left following the adventurer, a rail on the right with vitals, gear and what is nearby, and the log along the bottom.
The examine panel floats over the map and the targeting box over the foot of the rail, as Foundry's do, so neither hides what is being looked at or aimed at.

## The content model

Every content file is RON, and every file starts with a comment listing every field it may hold.
A monster is a short core, the things every monster has, and a list of named traits:

```ron
(name: "goblin conjurer", glyph: 'g', color: (0.6, 0.4, 0.8), hp: 10,
 attack: "1d3 slash", faction: "goblins",
 traits: ["sapient", "cowardly", "summons blades", "blinks"])
```

A trait is written once, in `traits.ron`, and bundles what the engine already knows how to do: wits, speed, sight, a flee threshold, resistances, armor, light, abilities, statuses held for life, and the words the examine panel says about it.
Lists add up across traits, and a single value set by two traits of one monster is a load error naming both, so a monster can always be read as its core plus its traits with nothing hidden in between.
An item lends traits to its wearer the same way, so a ring of the salamander and a salamander are fire-blooded for the same reason.
Deeper monsters simply list more traits.

## Floors

Each depth band has a biome, and each band draws its biome per run from two or three, so floor four is not always caves.
The bands as planned, with the biome the skeleton ships first:

| Floors | Biome (skeleton) | Other biomes (part 4) | Light |
| --- | --- | --- | --- |
| 1-2 | The Upper Halls: ruined masonry, rooms and corridors | the Cellars | dim, lantern and braziers |
| 3-4 | The Fungal Deeps: caves | the Spider Warrens | glowing fungus, teal and green |
| 5-6 | The Catacombs: crypts and ossuaries | the Sunken Library | candles, cold blue |
| 7-8 | The Drowned Cathedral: flooded halls, bridges | the Underlake | water shimmer, moonlight shafts |
| 9 | The Forge of Embers: lava and smithies | | firelight |
| 10 | The Throne: the boss's lair and its court | | the boss's own |

Generation mixes techniques per biome: accretion halls, cellular caves, crypt grids, lakes with bridges, lava rivers, and districts of one inside another.
Prefabs come in three kinds, each drawn from a weighted pool with bands and a budget per floor:

- **Flavour**: scenery that says what a place was, a library, a shrine, a goblin camp, a collapsed hall. Nothing to take.
- **Reward**: a treasure room, an armory, a shrine with a gift.
- **Risk and reward**: an altar vault where taking one caged item locks the others, a key on the floor opening a door elsewhere, a guarded hoard, a trapped corridor, captives in shackles who become allies when freed under fire.

The dungeon should feel lived in: monsters asleep, on patrol, at a fire or feeding, goblins and their ogre together, the remains of earlier adventurers with what they carried and a line of what they wrote, inscriptions, bones, blood, cobwebs, and small life that is only scenery.

## Monsters

Every kind has a role (brute, skirmisher, caster, swarm, ambusher, support, summoner), a tell and a trick.
Tricks by depth, as planned for part 3:

- **Floors 1-2, one trick:** rat (packs, flees when hurt), jackal (packs, fast), kobold (throws darts, cowardly), goblin (picks up and wears what it finds), cave spider (webs, venom), monkey (steals and runs).
- **Floors 3-4, two tricks:** goblin conjurer (spectral blades, blinks), goblin mystic (shields allies), myconid (slow, bursts into spores on death), pink jelly (splits when struck), vampire bat (erratic, drains), ogre (heavy blows, slow).
- **Floors 5-7, three tricks:** skeleton (knits back together unless burned or crushed), necromancer (raises the dead, blinks, cowardly), ghoul (paralysing claws), wraith (unseen in the dark, drains), will-o'-wisp (glows, lures, blinks), naga (venom, fast in water).
- **Floors 8-9, four tricks:** salamander (fire breath, fire-blooded, burning trail, glows), troll (regenerates, strong), golem (vast health, slow, reflects magic), the dar trio (blademaster, priestess, battlemage).

Mini-bosses are named monsters with an entourage and a lair, one or two a run from a pool: Grubnak the goblin king and his war band, the Widow in a web-choked cave of egg sacs, the Bone Choir, the Drowned Abbess in a flooded chapel, the Forgemaster at his anvil.
Each guards something worth the fight.

The final boss is drawn from a pool too, in part 3: the Drowned King, who raises the drowned dead; the Mother Below, who spawns; the Ember Wyrm, who breathes fire.
The skeleton ships the Drowned King alone.

## Items and builds

No experience and no levels: the adventurer at the bottom is the adventurer at the top with better things and fewer potions.
Part 2 built most of it, after Brogue, and what it left is under the parts below:

- **Enchanting is the currency.** Scrolls of enchanting are the most common scroll, and each adds one level to an item of the player's choosing. A build is where the scrolls went, so it is a commitment made one scroll at a time against what the dungeon has shown so far.
- **Strength** gates heavy weapons and armor; potions of strength are rare, and wearing something too heavy costs accuracy and armor.
- **Identification**: potions and scrolls wear a different face each run, a murky potion, a scroll titled XOMBA, and are known by use. Some are bad to drink and good to throw.
  A scroll read onto another thing is known the moment the pack asks which, since asking already tells it is one of the kinds that ask, and backing out of the choice spends nothing.
- **Staffs recharge, wands do not, charms do slowly.** Enchanting a staff raises both its power and its charges.
- **Weapons have shapes**: a dagger strikes anything unaware five times as hard, where any other blow strikes it three times as hard, an axe strikes everything beside you, a spear strikes two in a line, a mace staggers, a rapier lunges.
- **Armor is weight**: heavy armor is loud, and noise is what wakes a floor.
- **Artifacts**, carried by mini-bosses, are build-defining and have a cost.
- **Curses**: a cursed thing cannot be taken off, and some are worth wearing anyway.

Builds that should emerge, none of them chosen up front: blade and shadow (stealth and daggers), shield and steel (heavy armor, maces, shields), flame and ruin (fire staff and fire immunity, burning everything), storm and speed (spear, lightning, haste, blinking), bone and will (domination, conjuring, mercenaries and freed captives), venom and rot (gas, poison and protection from both).
They cross: fire and a web, lightning and water, a dominated ogre and a horn that strengthens allies.

## The town

Mereholt, for part 5: the Sunken Lantern tavern where mercenaries are hired and rumours about this run's dungeon are heard; a smithy for weapons and armor; an apothecary for potions and scrolls; a curio shop with rare, expensive stock; a temple that heals, lifts curses and identifies, for a price; townsfolk with a line each; and the Drowned Gate.
Gold is found below and spent here; shop stock is drawn per run.
The skeleton's town is the map, its lamps and the gate, with nobody to talk to yet.
Part 5's spec, `docs/superpowers/specs/2026-10-08-undermere-town-design.md`, is the content sheet: the keepers, their stock and prices, the temple's rites, the townsfolk's lines, this run's rumours, and the waystone.

## Mercenaries

For part 6: hired at the tavern from a few candidates drawn each visit, each a kind (sellsword, bowman, hedge witch, cleric, brute) with a name, traits (brave, greedy, loyal, drunk) and a kit.
At most two at once.
A mercenary follows, fights what the party fights, runs when badly hurt, drinks its own potions, picks up and wears gear better than its own, and follows the adventurer down every stair and through the gate.
A simple order key switches between follow, hold and fight freely.
They die for good, and the end screen remembers them.

## Score

Itemized, every line shown on the end screen and in the records:

| Line | Points |
| --- | --- |
| Depth | 100 for each floor reached, counted by the deepest |
| Slain | each kill's threat, the adventurer's and their hired mercenaries', the engine's `balance::threat` of what was killed, summed and rounded |
| Treasure | the gold found |
| Mini-bosses | 500 each |
| Victory | 5000 |

The numbers are a first cut and expected to move as the content arrives; the lines are the design.
The stats beside the score: turns, the deepest floor, kills and the deadliest thing killed, damage dealt and taken, potions drunk, items found, gold found, mercenaries lost, and the seed.
Kills, the deadliest and damage dealt are the company's, the adventurer's and their hired mercenaries', as the Slain line is; damage taken is the adventurer's own, and so is the Mini-bosses line, which pays for the blow.

## Presentation

Brogue's look, pushed further: every cell its own shade of its tile, light that colours glyph and background alike, water and lava that move, fungus and magic that glow, fire that spreads and smokes, gas that drifts, and an animation for every bolt, breath and blast, seen to arrive before it hurts.
Square tiles make every circle round, which Brogue's narrow cells never could.
The adventurer's lantern dims as they descend, so the dark grows with the danger.
The title screen is a painting that breathes: the black lake under stars, mist over the water, the town's windows lit, and the gate's stones glowing.

## The parts

1. **Skeleton**: the whole loop, thin. Title menu and run records in the engine, Foundry moved onto the engine's title menu, the town map, the gate, ten floors from the engine's current passes, a starter bestiary and armory in the trait model, the Drowned King, score, end screen, records, saving.
   Built on the `undermere` branch, finished 2026-10-04, as `docs/superpowers/specs/2026-10-02-undermere-skeleton-design.md` set out.
   It built the engine's title and records, with Foundry on the title; Mereholt at dusk and its gate; ten floors in five biomes, each seen dimly by its own ambient beyond a lantern that dims with depth; sixteen monsters and nineteen items in the trait model, an item lending its traits while worn; the Drowned King and his court, whose death wins; the score in five lines on the end screen and in the records; and saving, Continue and Main menu.
   Along the way the engine gained a status that lasts until it is cured, a mind that weighs its own blow against a worn one, a run's facts forgotten when it ends, and wide tiles that keep a screen over the map a letter to a cell.
   It left the examine panel's trait words to part 3, where monsters' identities are designed, since naming a monster's traits wants the look panel to wrap long lines.
   It left dark sight doing nothing, since every floor is dim rather than dark, until parts 3 and 4 bring true darkness back in places; the adventurer with no abilities of their own and the bow with no ammunition until part 2; and a font of the game's own to part 7, which part 2 brought forward.
   Two engine gaps it worked around went to `docs/TODO.md`: the engine's `FarthestExit` stopped at a shut door, so `dungeon.rs` measured the way down with a pass of its own until part 2 mended the engine's and took that pass out, and the `Doors` pass hangs two doors back to back where rooms sit two cells apart.
2. **Items and builds**: enchanting, strength, identification, staffs, wands and charms, weapon shapes, curses, artifacts.
   Built on the `undermere` branch, finished 2026-10-07, as `docs/superpowers/specs/2026-10-05-undermere-items-design.md` set out.
   It built the engine's half first: a game's own typeface, a use aimed at somewhere and a use made on another thing, things not yet known and things that will not come off, blows and shots that sweep or pierce and blows that land harder on the unaware, a status that takes its holder's turns and speed from a stat, ammunition lifted out of Foundry, and the farthest exit measured through doors.
   On that it built identification, a face for each kind each run, learned by using it or by a scroll of identify, and a worn thing's level learned by wearing it long enough; enchanting, with a scroll of enchanting on every floor; strength, which heavy gear wants and a potion of strength raises; and curses, a thing found below nought clinging once put on and showing its sign and not its size.
   While a worn thing's level is not known, the vitals, the look cursor's forecast and the targeting box's chance to hit count it at its kind's plain numbers, so neither tells the level that wearing it long enough would.
   A playtest found the old `+? chain mail` unreadable, so a thing whose level is not known is `unidentified chain mail`, a known one carries its level unless it is nought, `sword` and `+2 chain mail`, and the pack and the look cursor say what any thing of the kind is at nought, what is known of its level and how many more turns wearing it will take, with a line in the log the first time it goes on.
   The armory is ten weapons with their shapes, darts and javelins to throw and arrows for the bow, six suits of armor with their weight, a shield and a helm, six staffs, five wands, six rings, eleven potions and nine scrolls, with the effects of the game's own they need; its glyphs are drawn in DejaVu Sans Mono, which it ships, so its water, trees, gate and title read alike everywhere.
   A soak run by hand plays fifty seeds' floors under three plans for the scrolls of enchanting, and the content was tuned against what it counted.
   It left artifacts and charms to part 3, with monsters using what they carry; the ring of awareness waiting on hidden things for parts 3 and 4; the potion of darkness putting the drinker's lantern out rather than pooling true darkness, which wants a light that takes light away; the mace's stagger and the rapier's lunge; and to part 3's bestiary the finding that flat armor, enchanted, leaves the first eight floors nearly harmless.
3. **Monsters**: the bestiary by depth, mini-bosses and their lairs, the boss pool.
   Built on the `undermere` branch, finished 2026-10-09, as `docs/superpowers/specs/2026-10-07-undermere-monsters-design.md` set out.
   It built the engine's half first: a mind that uses what it carries, a status that sleeps until harm, a noise or a noticed foe wakes it, an actor that answers its own moments, hit, struck and died, through a remnant as a prop's death already does, an actor that offers a verb as a prop does, a mind that follows one leader and keeps with its allies while that one is gone, a body that turns a use back on whoever aimed it, the one corner rule a search and the move resolver now share, so a mind that opens doors is never refused the same diagonal twice, and the look panel wrapping a trait onto as many lines as it takes, which closes what part 1 left to this part for exactly that reason.
   On that it built the bestiary: about thirty kinds across the ten floors, each a short core and its tricks, known outright for a plain trait and a question mark for a trick until it is seen; the five mini-bosses planned, each in one of two lairs a run draws, never the same twice, with an entourage ready to follow its chief once freed to fight beside it; the boss pool of three, drawn for floor ten; and the five artifacts, one a mini-boss, that part 2 left to build.
   A floor now feels lived in: a share of its monsters sleep, falling with depth, a goblin war band comes together round its own fire, a beast stands over a feeding ground, a patrol walks a loop, and a shackled captive fights beside the adventurer once freed and follows it down every stair.
   A soak run by hand plays fifty seeds' floors to their ends, and the run is won far oftener for it, about one in seven where it was one in a hundred and fifty, with every final boss winnable; along the way the adventurer's health rose from forty to sixty, healing came back to the sheet's own numbers, and Grubnak, the Widow, the Abbess, the three bosses and their courts, the ogres, trolls, wights, skeletons and archers were each softened a step where the soak found one deciding runs.
   It left charms, which part 2 also assigned here, and the ring of awareness, assigned to this part and the next, still waiting on hidden things; a coloured light that cannot warm a tile with little of its colour, found in a camp's own fire and filed in `docs/TODO.md` for a part that works on how a game looks; and the player's own long action, which a freed captive gave only to a mind, filed the same way.
4. **Dungeon generation**: biomes per band, mixed techniques, the three prefab kinds in pools, a lived-in dungeon. Built on the engine's layout work.
5. **Town**: shops and gold, townsfolk, the temple, and whether the gate goes back.
6. **Mercenaries**: hiring, following, deciding, travelling.
7. **Polish**: a pass over everything for the look, the feel and the balance.
