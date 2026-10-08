# Undermere, part 4: the dungeon

Status: written 2026-10-08, on branch `undermere-dungeon`, cut from `layout` while part 3 was finishing on `undermere`.
The game design is `examples/undermere/DESIGN.md`, sections "Floors" and "The parts" (item 4); this is how they are built.
The engine work it stands on is `docs/design/layout.md`, the accretion layout that `layout` built for Foundry, and the research behind both is `docs/research/gridsage-mapgen.md`, cited below by section as "research 4.1".
The user asked for the design calls to be made without further questions, so every call below is the author's, each with its reason, made to be reviewed afterwards; the ones most worth a second look are gathered at the end.

## Goal

A floor of the Undermere should be worth looking at, worth exploring, and impossible to learn.
Ten floors deep, each drawn from a biome of its band, each grown by a layout plan drawn from several, held to metrics and thrown away when it falls short, and filled from pools of hand-made pieces larger than one run can see.
The dungeon looks like things happened in it: people lived, fought, fed, died and wrote on the walls here, and some of what they hid is still hidden.
And the ring of awareness, which part 2 left waiting, finally has something to find.

## What the player should never learn

The user's taste, and the research's central lesson (research 2.2, 10.1), is that a player learns rules and never layouts.
Foundry's corridor-first plan was refused for being one plan; `docs/design/layout.md` section 1 says why.
So unpredictability is stacked in six layers, any one of which a player might see through, and all of which together they should not:

1. **The biome is drawn per floor**, from two or three for the band, so floor four is caves, warrens or mines.
2. **The layout plan is drawn per floor**, from two or three recipes for the biome, which differ in map size and aspect, district mix, open share, loops and barriers.
3. **Districts** inside one floor mix shapes by region (layout.md 2.3), so a floor of halls has a knot of cellars in one corner, on some seeds.
4. **Barriers**, present by chance, bend whole regions round them (research 2.2, 10.1.4).
5. **Growth is wild and checked**: accretion is loose, and a floor that misses its recipe's bounds is thrown away and grown again from the next attempt of the same seed (research 4.1).
6. **Content is drawn from pools** with weights, bands, per-floor and per-run limits and exclusive groups (research 6.2, 6.5), so the rooms that matter are not where they were and often not there at all.

Two smaller rules serve the same end.
The way down is drawn among the farthest third of the floor by walking, not always the single farthest cell, because "head for the far edge" is itself a layout a player learns (research 2.2 says players learned that main exits sit at map edges).
And no technique belongs to only one biome: caves break into the halls, rooms are dug into the caves, so a technique does not tell a player where they are.

## The floors

Each floor draws its biome from its band, weights equal, from the floor's own stream, so a run's biomes are a pure function of its seed and any floor may be built first.
Floors are drawn independently, so both floors of a band may share a biome; the design's earlier "one biome per band" is dropped because a band of two floors that always differ is as learnable as one that always matches.
The most-seen floors get the most biomes: floors one and two are seen by every run, floor nine by few, which is the research's point about familiarity (research 5.5: Garrisons felt familiar after ten visits).

| Floors | Biome | How it is grown | Light |
| --- | --- | --- | --- |
| 1-2 | **The Upper Halls** (skeleton): ruined masonry | rectangles and crosses behind doors, corridors, a "ruin" district where small caves of rubble break in | dim and warm; braziers |
| 1-2 | **The Cellars**: storerooms under the halls | small rooms, crawlways one cell wide, root-broken cave pockets; tight, many dead ends that pay | darker; a few hung lanterns |
| 1-2 | **The Watch Barracks**: the drowned kingdom's garrison | uniform cells behind doors, cross-shaped mess halls, a parade hall joined by a wide mouth | torch sconces and braziers |
| 3-4 | **The Fungal Deeps** (skeleton): caves | large and medium caves joined by mouths, loops between them | glowing fungus, teal and green |
| 3-4 | **The Spider Warrens**: web-choked tunnels | many small caves and one-cell tunnels, low open share, webs | truly dark: ambient nought, so the lantern is the light |
| 3-4 | **The Old Mines**: dug galleries in rock | rectangular chambers among cave pockets on straight two-wide galleries with many loops (research 3.4) | goblin lanterns, glinting ore |
| 5-6 | **The Catacombs** (skeleton): crypts and ossuaries | small square crypts behind doors on long one-wide galleries, ossuary halls | candles, cold blue |
| 5-6 | **The Sunken Library**: halls of shelves | long rectangular stacks, cross-shaped atria, reading rooms, two-wide passages, seeping pools | pale violet witchlight, water shimmer |
| 7-8 | **The Drowned Cathedral** (skeleton): flooded halls | naves joined by mouths, transept crosses, chapels, cloister passages; lakes of deep water in the naves, bridges over them | moonlight shafts, water shimmer |
| 7-8 | **The Underlake**: the lake under the lake | big caves where lakes cover much of the floor, islands joined by bridges and causeways | glowing water, cold blue-green |
| 9 | **The Forge of Embers** (skeleton): lava and smithies | smithy rooms among ash caves, lava lakes with ash shores, iron bridges | firelight |
| 9 | **The Ashen Galleries**: cooled flows and live vents | caves of obsidian and cinder, lava lakes, few rooms | ember glow and dark between |
| 10 | **The boss's floor**, following part 3's draw: the Drowned Throne (the Drowned King: halls standing in black water), the Brood Hollow (the Mother Below: caves round a great cavern), the Wyrm's Hall (the Ember Wyrm: a pillared hall of burning pools) | a compact place grown round the throne's piece, which is laid first and far from the way in | the boss's own |

Biomes become data, `assets/biomes.ron`: a name, the floors it may stand on, a weight, its tiles, its recipes with weights, its ambient and light color, its lakes, its decor, its dressings, and the monsters it favors.
They were a closed enum in `dungeon.rs`; twelve biomes with a dozen fields each is content, and every content file in Undermere is RON.
A biome's favored monsters multiply their spawn-table weight by three on its floors (spiders in the warrens, goblins and kobolds in the mines, wisps and wraiths in the library, naga and the drowned under the lake), so a biome is also told by what lives in it.

Floors grow from the skeleton's 70 by 40, which is barely more than the map view of about 62 by 37 tiles.
Each recipe names its size, between 84 by 50 and 110 by 60, about one and a half views a side, and the Fungal Deeps' "band" recipe is 120 by 40, a long cave the floor follows (research 3.3, guided caves).
The throne floors are 72 by 44, since the fight there is the point and the walk to it is not.
Larger was considered and refused for now: Undermere has no clock that makes leaving a floor unexplored a choice (research 9.3), so a larger floor is more walking, not more decisions.

## How a floor is grown

Every floor goes through `Accrete`, `OpenLoops`, `Survey` and `Require`, as Foundry's decks do, and the chain is retried up to forty attempts with `Chain::run_retrying`.
`Rooms`, `Bsp`, the whole-map `CellularCave` and Undermere's own `JoinCaves`, `ThroneHall` and `Lair` passes leave the floors.
They are retired because none of them publishes areas, so nothing after them can ask which room is a dead end or far from the way, and pools, secrets and dressings all key on exactly that.

A floor's chain, in phase order:

1. **Structures**: `Accrete` grows the recipe's districts, laying this floor's grown pieces (vaults, lairs, the throne) late in growth, inside the recipe's barriers; `OpenLoops` cuts the longest detours through; `Lakes` lays the biome's water or lava without cutting the walkable floor in two.
2. **Connect**: `Bridges` spans a liquid where walking round it is long and the span is short.
3. **Exits**: the way in, clear of every piece; the way down, among the farthest third and clear of every piece; `Survey`.
4. **Finish**: `HideOpenings` makes this floor's secret doors and hidden rooms; the pool's inside picks are stamped into the areas their preference asks for; `Dress` dresses every area nothing else took; Undermere's decor, traps, hidden caches and paint; `Underfoot`; `Require` last.

A floor's floor tiles are painted after growth by Undermere's own pass, from each area's shape and district names, so a cave district inside the Upper Halls is floored with rubble and earth and its rooms with flagstones.
The engine records those names and reads no meaning into them (layout.md 2.1); painting by them is the game's.

### The techniques

Growth is accretion throughout, and the techniques are shapes and districts in `assets/layout.ron`:

- **Rooms and halls**: the engine's rectangles and crosses, joined by doors, arches or mouths.
- **Corridors**: the engine's passages, one to three wide, bending by chance.
- **Caves**: a new engine shape, a piece drawn by the cellular automaton applied at random cells (research 3.1) within a box, kept to its largest region, and joined like any piece; caves accreted this way are individual areas the survey can rate and the pool can fill, which is the reason research 3.2 gives for keeping caves separate.
- **Composites**: districts of caves and rooms in one recipe, the research's mines and "hidden base" (research 3.4).
- **Lakes and bridges**: a new engine pass pair, water in the cathedral, the library and under the lake, lava in the forge and the galleries.
- **Grown pieces**: a prefab laid as its own piece through its own entries, a new engine seam the layout design left for this (layout.md 2.1: "the trait is the seam it will use").

A tunneler was considered and not built.
Accretion's passages already vary width and bend, districts already vary a region's taste, and the research says plainly that a tunneler's step rules are not stated anywhere (research 2.4), so building one is designing a second growth algorithm to maintain for no look accretion cannot give.

### The recipes

Two or three per biome, twenty-nine in all, in `assets/layout.ron`, each with its own size, districts, open share, loops, barriers and requirements.
The bounds every recipe states (research 4.1): open share at least and at most; the fewest areas of the shapes that make the biome what it is; the fewest loops; the largest share of dead ends; and the shortest walk from the way in to the way down, a third of the map's longer side or more.

| Biome | Recipes |
| --- | --- |
| Upper Halls | `great halls` (halls by mouths, few corridors), `wings` (two barriers split the map into wings met at their ends), `galleries` (long passages, small chambers, many loops) |
| Cellars | `storerooms` (small rooms, crawlways), `burrowed` (half the districts root caves) |
| Watch Barracks | `blocks` (cell districts of uniform rooms), `parade` (one great hall district, crosses round it) |
| Fungal Deeps | `grottoes` (large caves, open 42 percent), `band` (120 by 40, guided), `knot` (many small caves, many loops) |
| Spider Warrens | `warren` (small caves, tunnels, dead ends up to 60 percent), `nest` (a district of large caves among small ones) |
| Old Mines | `workings` (chambers on galleries, five loops at least), `deep seam` (caves first, chambers rarer, a barrier) |
| Catacombs | `grid` (crypt districts of three to five cell squares), `ossuaries` (halls and crosses, galleries between) |
| Sunken Library | `stacks` (long halls in parallel districts), `atrium` (crosses and reading rooms) |
| Drowned Cathedral | `nave` (great halls, deep lakes, bridges), `cloister` (chapels round a barrier, passages two wide) |
| Underlake | `shore` (caves along a great lake), `archipelago` (lakes over most of the floor, islands bridged) |
| Forge of Embers | `smithies` (rooms among ash caves, lava lakes), `smelter` (a hall district round a lava lake) |
| Ashen Galleries | `flows` (long caves, lava lakes), `vents` (small caves, many small lava pools) |
| Throne floors | `drowned throne`, `brood hollow`, `wyrm's hall`, one per boss |

Recipes are read before they are called done, as Foundry's were: `undermere --floor N --seed S` prints a floor as glyphs, and sample floors across seeds are looked at by eye for outliers (research 4.5), because a floor that meets every bound can still read badly.

## Pools

The design's three kinds stand: flavor, risk and reward, reward.
(The design wrote "flavour"; identifiers and new prose are American, per the agent guide.)
Every piece is drawn from one pool file, `assets/pool.ron`, which the engine owns as it owns the layout file: entries, named weights, budgets, and the checks.

### How a pool draws

- **Named weights**, after research 6.2: `rare` 5, `uncommon` 20, `common` 50, `frequent` 100, declared at the top of the file so they read easily and retune at once.
- **Bands**: the floors an entry may stand on. **Where**: the biome names it may stand in, or any.
- **Limits**: at most one of an entry per floor; a per-run limit on every entry, two for a shared flavor and one for most else.
- **Groups**: entries in one exclusive group never share a floor (two altar rooms, say).
- **Placement**: `Grown(join)`, laid by `Accrete` as its own piece through the entries its file marks, joined by a door, an arch, a locked door, a sealed gate or a secret; `Inside(shapes)`, stamped inside an area drawn by one of the named shapes that fits it; `Dressed`, a dressing that fits an area of any size.
- **Preference**: `Secluded` (the area farthest from the route, research 4.3: "the more secluded a room, the more likely it will hold something of value"), `DeadEnd` (layout.md 1.3: a dead end pays), `Hidden` (an area behind a secret), `Junction`, or anywhere.
- **Budgets** per band and kind, a range per floor; a pick of a kind its budget marks required fails the floor when it cannot be placed, so a run never quietly loses a vault; a flavor pick that fits nowhere is dropped.

The whole run's picks are drawn at run start from the seed, one list per floor, against the run's biomes, so every per-run limit and group is checked in one place and no floor's contents depend on which floor was built first (research 6.5, which assigns uniques to maps at world generation for the same reason).
A floor's attempts then vary its layout and never its picks.

### Budgets

| Floors | Flavor | Risk and reward | Reward |
| --- | --- | --- | --- |
| 1-2 | 3-5 | 1 | 1-2 |
| 3-6 | 3-5 | 1-2 | 1-2 |
| 7-9 | 2-4 | 2 | 1 |
| 10 | 1-2 | none | none |

A run draws about thirty flavor picks, thirteen risks and twelve rewards; against pools of 43, 22 and 16, filtered by band and by the run's biomes, a run sees about half the risk pool and under half the flavor, which is the design's fifth pillar.
The mix leans to reward and risk over flavor, as research 6.3's game-wide mix does, because flavor is also carried by the dressings in every room the pool did not take.
Part 3's lairs keep their own draw, two a run, and become grown pieces; the throne is the tenth floor's grown piece.
The item budget the floors already drop, and the chests, fall to nought or one a floor, retuned in play, since rewards now carry most of what is found.

### Flavor: 43

Nothing to take; what a place was.
`D` is a dressing that fits any area, `P` a fixed piece stamped inside one.
Shared entries stand on floors 1-10 in any biome, at a run limit of two; a biome's own at one.

| Piece | Where | Weight | What it is |
| --- | --- | --- | --- |
| Collapsed Ceiling (D) | any | common | heaps of rubble and a fallen beam across part of a room |
| Bone Scatter (D) | any | common | gnawed bones and a cracked skull where something fed |
| Cold Campsite (P) | any | uncommon | a dead firepit, two bedrolls, a broken pot: a party camped here once |
| Last Words (P) | any | uncommon | a slumped skeleton beside a wall scratched with its last line |
| Fallen Statue (P) | any | uncommon | a noble of the drowned kingdom toppled in three pieces |
| Wayside Shrine (P) | any | uncommon | a niche, an empty bowl, rusted coins pressed into old wax |
| Rat Midden (D) | floors 1-6 | common | straw, rinds, droppings and a hole in the wall |
| Web-Choked Corners (D) | any | common | old cobwebs in every corner |
| Seep (D) | floors 1-8 | uncommon | a shallow corner pool, dripping, moss round it |
| Chalk Warnings (D) | any | rare | arrows and marks left by the last party, one readable |
| Feasting Hall (D) | Upper Halls | common | long tables in rows, toppled benches and goblets |
| Guard Room (D) | Upper Halls | common | empty weapon racks, a table with dice still on it |
| Steward's Audience (P) | Upper Halls | uncommon | a lesser throne under rotted banners |
| Wine Cellar (D) | Cellars | common | racks of casks, smashed barrels, a stained floor |
| Root Cellar (D) | Cellars | common | roots through the walls, earth heaps, old sacks |
| Coal Store (P) | Cellars | uncommon | heaped coal, a shovel, black dust underfoot |
| Bunk Room (D) | Watch Barracks | common | bunks along the walls, footlockers broken open |
| Mess Hall (D) | Watch Barracks | common | trestle tables in rows and a cold hearth |
| Drill Yard (D) | Watch Barracks | uncommon | straw dummies and posts, a bare rack |
| Glowcap Ring (P) | Fungal Deeps | common | a ring of glowcaps round bare earth |
| Puffball Grove (D) | Fungal Deeps | common | tall fungus stalks to walk between |
| Moss Garden (D) | Fungal Deeps | uncommon | the floor gone thick with moss, dripping |
| Husk Gallery (D) | Spider Warrens | common | cocooned husks hanging along the walls |
| Silk Curtains (D) | Spider Warrens | common | sheets of old web strung across a cave in rows |
| Old Egg Cases (D) | Spider Warrens | uncommon | split egg cases, long empty |
| Cart Track (P) | Old Mines | common | rails and a tipped ore cart |
| Shored Gallery (D) | Old Mines | common | timber props along the walls and a fallen beam |
| Ore Seam (D) | Old Mines | uncommon | veins in the wall that glint in the lantern |
| Ossuary Wall (D) | Catacombs | common | skulls stacked in wall niches |
| Tomb Row (D) | Catacombs | common | sarcophagi in rows |
| Embalmers' Room (P) | Catacombs | uncommon | slabs, jars and rotted linen |
| Reading Room (D) | Sunken Library | common | desks in rows and dead lamps |
| Fallen Stacks (D) | Sunken Library | common | toppled shelves and drifts of pages |
| Orrery (P) | Sunken Library | uncommon | a broken brass orrery, its spheres fallen |
| Choir Stalls (D) | Drowned Cathedral | common | carved stalls in rows facing a ruined screen |
| Baptistery (P) | Drowned Cathedral | uncommon | a font overflowing into the floor |
| Fallen Bell (P) | Drowned Cathedral | uncommon | a cracked bell in a crater of flagstones |
| Stilt Hut (P) | Underlake | common | a fisher's hut on stilts, nets drying on nothing |
| Shell Midden (D) | Underlake | common | heaps of opened shells |
| Smithy (D) | Forge of Embers | common | anvils, quench troughs and tongs along the walls |
| Slag Heaps (D) | Forge of Embers | common | heaps of slag, still warm |
| Obsidian Spires (D) | Ashen Galleries | common | glassy spires the light breaks on |
| Cinder Drifts (D) | Ashen Galleries | common | the floor deep in ash |

### Risk and reward: 22

A prize behind something that bites.
Each run limit is one unless given.

| Piece | Floors, where | Weight | Placed | What it is |
| --- | --- | --- | --- | --- |
| Caged Altars | 2-9 | uncommon | grown, door | three caged things on altars; take one and the other cages slam shut |
| The Locked Vault (run 2) | 1-9 | common | grown, locked | a vault door no hand opens; its key lies elsewhere on the floor, often guarded |
| The Guarded Hoard (run 2) | 1-9 | common | inside, secluded | gold and a chest with a band of the floor's own asleep round it |
| The Trapped Corridor | 2-9 | uncommon | grown, arch | a corridor of hidden plates to a room that holds the prize |
| Captives in Chains (run 2) | 2-9 | uncommon | inside | part 3's captives, shackled under guard; freed, they follow |
| The Rat Trap | 1-6 | uncommon | grown, door | a thing on a pedestal; lift it and the walls crack open on niches full of vermin |
| The Flooding Vault | 3-9, not the forge or galleries | uncommon | grown, door | lift the prize and water pours in through grates, rising to deep |
| The Tinder Room | 1-8 | uncommon | inside, dead end | dry straw over the floor, a lit brazier by the door, the prize at the back |
| The Rotten Floor | 1-8 | rare | grown, arch | planks over a drop; lift the prize and the floor gives way to the floor below |
| The Gas Chamber | 3-9 | uncommon | grown, door | a hidden plate in the doorway vents caustic gas into the room |
| Gallery of Darts | 2-7 | uncommon | grown, mouth | statues flank a hall and hidden plates fire darts from them |
| The Spider's Larder | 3-6, caves | common | inside | cocooned bodies to cut open for what they carried, spiders overhead |
| The Goblin Stockade (run 2) | 2-6 | common | inside, large | a palisaded camp round a fire, sleepers, a watch and their loot pile |
| The Drowned Reliquary | 6-8, water biomes | uncommon | grown, arch | a reliquary on an island, reached by a submerged causeway that must be spotted |
| The Sealed Vault | 2-9 | uncommon | grown, sealed | a stone gate; a concealed lever elsewhere on the floor raises it |
| The Bone Circle | 5-8 | uncommon | inside | a ring of bodies that rise when it is crossed; a staff or scroll at its heart |
| The Warded Hall | 4-9 | uncommon | grown, door | warding glyphs that cry out when crossed, waking the floor; an altar beyond |
| The Brood Chamber | 3-7 | uncommon | inside, caves | part 3's egg sacs that hatch when disturbed, the prize among them |
| The Sleeping Giant | 3-6 | uncommon | grown, passage | an ogre asleep across a narrow way; the prize behind him |
| The Paralysis Vestibule | 6-9 | rare | grown, door | a hidden vent in the vestibule holds the entrant fast while what waits within wakes |
| The Last Stand | 2-9 | uncommon | inside, secluded | an adventurer's remains among hidden traps, pack still full, killers asleep nearby |
| The Collapsing Gallery | 1-6, halls, cellars, mines | rare | grown, two entries | lift the prize and the way you came falls in |

Each carries a tell at its entrance, an arch, carvings or a door unlike the floor's others, so it is seen before it is understood (layout.md 1.6, anticipation over surprise; research 7.4, billboarding).

### Reward: 16

Unguarded, and placed where seclusion says.
Each run limit is one unless given.

| Piece | Floors, where | Weight | Placed | What it is |
| --- | --- | --- | --- | --- |
| Treasure Room (run 3) | 1-9 | common | inside, secluded | heaps of gold and a chest of coin |
| Armory (run 2) | 1-9 | common | dressed, secluded | racks along the walls with two weapons or armors a floor deeper |
| Apothecary's Shelves (run 2) | 1-9 | common | dressed | shelves with two or three potions |
| Scriptorium (run 2) | 1-9 | common | dressed | lecterns in rows with two scrolls |
| Hidden Cache (run 3) | 1-9 | common | inside, hidden | a chest two floors deeper in a room behind a secret door |
| A Fallen Adventurer (run 2) | 1-9 | common | inside, secluded | remains with a full pack and their last line |
| Wizard's Study | 2-9 | uncommon | inside | a desk with a staff or a wand, books |
| Jeweller's Niche | 2-9 | rare | grown, arch | a cubby in a corridor wall with a ring, seen in passing (research 7.4) |
| Healing Spring (run 2) | 1-9 | uncommon | inside | a spring that heals fully, once |
| Enchanter's Altar (run 2) | 1-9 | uncommon | inside, dead end | an altar holding a scroll of enchanting |
| The Cold Forge | 3-9 | uncommon | inside | an anvil that tempers a worn weapon or armor one level, once |
| Quartermaster's Store (run 2) | 1-9 | common | dressed | bundles of arrows, darts and javelins, a bow |
| Overturned Wagon | 1-4, halls, cellars, barracks, mines | uncommon | inside | a spilled cart with gold and a thing a floor deeper |
| Quiet Reliquary | 4-9 | rare | grown, door, secluded | one thing three floors deeper on a pedestal in a candlelit chapel |
| Library Nook (run 2) | 1-9; common in the library | uncommon | inside | a book of the kingdom's history and a scroll of identify |
| Moonwell | 5-9 | rare | inside | a well whose water tells what one carried thing is |

## Hidden things

Part 2 gave the ring of awareness fifteen points a level on the chance a turn of spotting what is hidden, and nothing to spot.
This part hides six kinds of thing, each found by the one rule below, with stepping on it the other way for traps.

| Thing | How it is hidden | How it is found | Per floor |
| --- | --- | --- | --- |
| Secret door | a loop's door, never one on the way from the stairs to the stairs, walled over in the wall it stands in | spotted; then a door like any other | 1-3, from floor 2 |
| Hidden room | a dead-end room's only door, walled over; the hidden cache and some rewards stand inside | spotted | 0-2, at least one when the floor's picks ask for it |
| Trap | a prop with a trigger on being entered, hidden | spotted; or sprung, which shows it; or seen sprung by a monster | 0-1 on floor 1, rising to 3-5 on floors 7-9 |
| Concealed lever | a hidden prop offering "pull", only with the Sealed Vault | spotted; pulling raises the vault's seal | 0-1 |
| Loose flagstone | a hidden container offering "search", a little gold or a potion | spotted | 0-2 |
| Submerged causeway | cells of shallow water drawn as deep, a group found together, only in the Drowned Reliquary | spotting any cell of it finds all | 0-1 |

The traps, unlocked by depth: dart (from floor 1), alarm glyph that cries out and wakes the floor (2), net that holds (3), trapdoor to the floor below (3, never on floor 9, so the throne is entered by its stairs), caustic vent (4), fire jet (5), paralysis vent (6).
They stand on corridors, chokepoints and doorways the survey names, never within six cells of the way in nor on a way through.
Their spot chances run from twenty percent for a dart plate down to eight for a paralysis vent; a secret door is six and a hidden room's door five; numbers are a first cut for the soak.

### The one rule of finding

A hidden thing in the adventurer's sight is rolled for every turn, as the engine already rolls hidden props (`docs/design/props.md` section 5), with three changes:

- **Nearness**: the chance is whole within two cells, half out to seven, and nothing beyond, so a secret is found by being near it, not by glancing down a hall.
- **Keenness**: a finder's own bonus, `Keen`, is added to every roll, so the ring of awareness adds to the finder and stops rewriting every hidden prop's chance as Undermere does today.
- **Stillness**: a turn spent waiting triples the chance, so waiting is searching.

There is no search key.
The engine refused one because pressing it at every wall is a chore (props.md section 5), and that reason stands: nothing is found only by searching, since walking past still rolls.
But a player who suspects a wall (a dead end that should go somewhere, a gap in the map between two rooms) has a lever, the same wait key they already rest with, and a ring of awareness makes it a strong one.
This is the author's reconciliation of the user's list (search, the ring, stepping on it) with the engine's refusal, and the first decision worth reviewing.

Monsters do not use secret doors: until found, a secret is wall to everything, so a hidden room is a reward pocket and never a monster's back door.
Cogmind uses hidden doors the other way, to give hostiles ambush routes (research 2.1); that is refused here because a door a monster walks through in plain sight is not secret, and showing it then is a second rule.
Monsters do spring traps, which is the lived-in half of a trap: a fire jet seen from across the floor, a goblin caught in a net.
A trap fires once and stays sprung, as the engine's props already do, so a wandering floor does not burn itself out before the adventurer arrives.

### What the engine provides, and what the game adds

The engine already hides props, rolls for them in sight, fires a hidden trap, and keeps a sprung or spotted prop as it was through a save.
It gains, theme-free: cells that hide what they are, made by a mapgen pass that hides openings safely and kept as entities spotted by the same roll and saved; a group found together; nearness, keenness and stillness in the roll; and a `Found` message for both kinds.
The game adds: how many of each, their chances, the trap props and their effects, the lever and its seal, the flagstones, the causeway, the trapdoor's fall, the ring's `Keen`, and the log's words.

## A lived-in dungeon

Part 3 made the people lived-in: sleepers, patrols, camps, feeders, captives, lairs.
This part makes the places lived-in:

- **Every room says what it was.** Areas the pool did not take are dressed from their biome's weighted dressings, including "nothing" at a share (research 7.2, six compositions with one empty; research 7.1, leave floor empty for tactics).
- **Decor by biome and by area**: rubble in the halls, moss creeping out of the caves, ash in the forge, cobwebs in corners, bones in the catacombs, blood smeared from a doorway into a dead end where a body lies.
- **The dead who came before**: Last Words, A Fallen Adventurer and The Last Stand each leave remains with a line they wrote, drawn from `assets/writings.ron`, never the same line twice a run.
- **Inscriptions**: some walls carry writing that offers "read", a line of the drowned kingdom's history from the same file, so the place has a past a player can piece together.
- **Level feelings**: a floor holding a piece with a feeling logs it on arrival ("You hear water pouring somewhere below", "The air smells of old smoke"), so the rare floor is anticipated (research 8.2, the level feeling; layout.md 1.6).
- **Small life**: moths that circle lights and blind cave rats that flee, harmless, in the trait model, belonging to no side and worth no score.

Per-area darkness was considered and left out: the engine lights a map with one ambient (lighting.md section 2), and the Spider Warrens' whole-floor dark is enough true darkness to give part 3's shrouded wraith and the adventurer's dark sight their use.
A surface layer for stains drawn under a glyph, as Brogue has, was considered and left to part 7: blood and moss here are tiles and scenery props.

## Engine work

Eight slices, each in the crate that owns the loop, each theme-free, each with a `CHANGELOG.md` line, and each reviewed per task.
The first three extend `layout`'s growth; the rest are the pieces `docs/design/layout.md` section 0 names as the next work, the director and the furnisher, cut to what Undermere needs, and the secrets the props design deferred.

### 1. A cave is a shape

`layout.md` section 9: "Caves and any shape that is not built of rectangles: a `Shape` a later slice adds."
`Cave` draws a piece by the randomly applied automaton of research 3.1 inside a box of its ranges, keeps its largest region, and takes its sites from the rock just outside its rim, facing out.
The layout file gains `Cave(w:, h:, fill_pct:, visits_pct:)`, with its ranges checked at load as the others' are.
A cave is a room for the pool, so a dressing or a piece can stand in one.

### 2. A recipe may lay barriers

A barrier is a segment of rock growth may not cross, given in shares of the map so one recipe builds at any size, present at a chance and shifted by a jitter (research 2.2, random barriers).
`Accrete`'s fit check treats a barrier cell as it treats the border.
A barrier that would reach from one edge of the map to the opposite one is refused at load, since growth could never cross to the far side.

### 3. A piece grown from a prefab

A prefab's legend gains `Entry`, a cell of its outer wall where it may be joined.
A prefab is then a `Shape`: its body every cell inside its own wall, its sites its entries, turned and mirrored at random.
`Accrete` takes the pieces a place must hold, each with its join, and lays them once growth has passed a share the recipe names, so they attach to the outer frontier rather than the first room; a place that ends growth with one unlaid fails, for `run_retrying` to grow again.
A grown piece emits its `Stamped`, so its slots fill as any stamp's do, and its area remembers the piece's name.
Joins gain `Locked(tag)`, a door only an opener carrying the tag opens, and `Sealed`, a gate nothing opens until the game lifts it; a secret is the next slice's.
`FarthestExit` gains `clear_of_stamps(n)`, as `RandomStart` has, and `among_farthest(pct)`, drawing the way down from the farthest share by walking.

### 4. Lakes and bridges

`Lakes` lays blobs of a given tile over the open floor and the rock beside it, refuses any blob that would leave the walkable floor in two, and may wreath each with a shore tile at a chance.
`Bridges` lays a given tile straight across a liquid where the span is at most its longest and walking round is at least its detour.
Undermere's own `Patches`-for-water and `Rim` passes do the first half of this today, which is the sign it is a loop a game writes; Foundry's coolant could use it.
The research is Cogmind's and Cogmind has no lakes, so this slice rests on Brogue's practice, which designs lakes against the connectivity of the floor and bridges what they cut.

### 5. Cells that hide what they are: the generator's half

`HideOpenings` walls over openings that are safe to hide and records each as a `Secret`: its cell, the tile it really is, its chance to be spotted, its group, and the areas it joins.
Safe means two things only: a loop's opening, whose closing leaves the area graph connected; or the one opening of a dead-end room that holds neither way.
It never hides an opening on the walk from the way in to the way down.
`PlaceBuild` carries the secrets out, as it carries spots.
Research 4.1 lists the prevalence of hidden corridors as a requirement Cogmind checks, and research 2.1 says hidden doors are its only direct room-to-room links; here they are cut from loops, so a secret is always a shortcut or a pocket and never the only way on.

### 6. Cells that hide what they are: finding them

A `Secret` is an entity on its map with a position, spawned with the place and saved.
The spotting roll in `PropsPlugin` rolls for hidden props and secrets alike, from the same stream, with nearness, a finder's `Keen`, and stillness as above, set on the plugin.
A secret found sets its cell to the tile it is, finds the rest of its group, and is gone; a `Found` message says what was found, by whom and where, for a prop or a cell alike.
Undermere's rewriting of each hidden prop's chance from the ring goes, replaced by `Keen`.

### 7. The pool

`rl-rules` owns the pool file and its checks, and the draws, pure and tested with no `App`: a run's picks for all its places at once, from the seed and each place's band and tags, under weights, bands, per-place and per-run limits, groups and budgets.
`rl-mapgen` gains the pass that stamps inside picks into the areas their preference names, reading `Surveyed` and the secrets, and fails the place on a required pick it cannot fit.
`rl-bevy` turns a place's picks into the grown pieces `Accrete` takes and that pass.
This is `docs/TODO.md`'s "A furnishing pool with bands and a budget", and the "director" of `layout.md` section 0, cut to pieces and their limits; rare events and states are not here.
Nothing in an entry is a category the engine reads: a kind is a string the game names in its budgets (prefabs.md section 8 refused stakes and categories on a prefab, and the kind lives on the pool's entry, not the piece).

### 8. Dressing an area of any size

`docs/TODO.md`'s "Furnishing a room of any size" and "Furnishing a map that is not rooms", for areas the layout publishes.
A dressing is the prefab legend and rules for where its things go: along walls, in corners, in the center, in rows with aisles, scattered, at a density, with a share of the floor repainted.
`Dress` dresses every area no stamp took from a weighted list by district, including nothing, on the area's own cells, so a cave dresses as well as a hall; it never stands a thing on or beside an opening and never cuts an area in two, checked by flooding the area before and after each thing and taking back the one that cut it.
Its slots are emitted as a stamp's, so per-cell streams, the save, the coverage report and the load checks come with them.
This is the furnisher of `layout.md` section 0 and research 7.2's composition system.

## Testing

Each engine slice in its crate, with no `App` where the loop allows, over seed ranges at two sizes as the layout's own tests are.
The game's, over seeds 0 to 200 unless said:

- every floor of every biome and recipe builds within forty attempts and meets its recipe;
- every open cell is walkable from the way in with every secret found and every lock and seal opened, and the way down with none of them;
- every hidden thing stands on or beside a cell walkable from the way in once the secrets are found, and every trap is off the stairs and six cells from the way in;
- a run's biomes, recipes and picks are the same whichever floor is built first;
- every biome of every band and every recipe of every biome is drawn over the range, none more than two thirds of its biome's floors;
- no per-run limit is exceeded and no group shared on any run over seeds 0 to 500, and every pool entry appears on some run;
- a secluded pick's area is on average farther from the route than the floor's areas;
- the way down is never inside a piece, and its walk from the way in meets the recipe's bound;
- each risk-and-reward mechanism in its own test: one caged thing taken shuts the rest; the flood rises; the rotten floor drops the taker off the stairs of the floor below; the lever raises the seal; the key opens its own vault; the glyph wakes the floor; the circle raises its dead;
- the ring of awareness still adds fifteen a level, now through `Keen`; a secret beside a waiting adventurer is found within twenty turns on nearly every seed at base chances;
- part 3's lair, throne and population tests pass unchanged in what they assert.

## Decisions most worth reviewing

1. Waiting searches, and there is no search key (Hidden things).
2. Biomes are drawn per floor, not per band, and floors one and two have three biomes (The floors).
3. No tunneler; caves, composites and corridors are all accretion (How a floor is grown).
4. Secrets are walls to monsters, and monsters spring traps (Hidden things).
5. The way down is drawn from the farthest third, not the farthest cell (What the player should never learn).
6. The whole run's pool picks are drawn at run start; a floor's retries vary only its layout (Pools).
7. Floors grow to about one and a half views a side, not the research's two to four (The floors).
8. The furnisher is built now, as a dressing, rather than authoring every flavor at fixed sizes (Engine work 8).
9. Biomes become data in `assets/biomes.ron` (The floors).

## Not in this part

The town and gold (part 5), mercenaries (part 6), every cell its own shade and a surface layer for stains (part 7), per-area darkness and a light that takes light away, rare floor-wide events and whole-piece states (research 8.4, 6.4), and rivers.
