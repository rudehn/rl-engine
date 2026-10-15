# Undermere: the strata and the first landmarks

Status: written 2026-10-13, from a design interview with the user; not yet built.
The game design is `examples/undermere/DESIGN.md`, section "Floors", which this changes; the engine work it stands on is `2026-10-13-landmarks-design.md`.

## Goal

The floors should say where the adventurer is.
Today depth only picks a band of biomes, and the bands are not in an order a reader of the premise would expect: the cathedral and the library, buildings of the kingdom, sit deeper than the caves and mines.
And a floor is not remembered by anything on it.

## The premise, kept

The Drowned Gate opens onto the kingdom that sank under the lake; that premise stays.
What it gains is a direction: **depth is time**.
Near the top are the kingdom's streets and grounds, below them its tombs and foundations, deeper still the places that were there before the first stone, and at the bottom the oldest thing of all, which drowned it.

So a forest belongs in the Undermere twice over: the royal hunting wood sank with the kingdom, strange for an age under the lake, and a far older wood stood before the kingdom did.

The floors stay a tour: each depth window draws from its own pool of themes, as today.
A theme is its look, its layout and its terrain, and terrain may carry real properties, blocking movement or sight, or costing more to cross, as the engine's `TileProps` already allow; there is no rule that changes a whole floor.

## The strata

| Floors | Stratum | Themes |
| --- | --- | --- |
| 1-2 | The Drowned Town | Upper Halls, Cellars, Watch Barracks, **Old Town**, **Harbour** |
| 3-4 | The Palace and its Grounds | Sunken Library, Drowned Cathedral, **Royal Wood**, **Gardens and Menagerie** |
| 5-6 | Beneath the Kingdom | Catacombs, Old Mines, Fungal Deeps, Spider Warrens, *Cisterns (later)* |
| 7-8 | Before the Kingdom | Underlake, *Elder Wood (later)*, *Barrows (later)* |
| 9 | The Oldest Deep | Ashen Galleries, Forge of Embers |
| 10 | The thrones | unchanged |

Bold is new in the first wave; italic is new in a later wave.
The Sunken Library and the Drowned Cathedral move up to 3-4; the Old Mines, the Fungal Deeps and the Spider Warrens move down to 5-6.
The Forge of Embers is recast as the forge of the people before the kingdom, in its moods, rumour and lore, not its layout.
Every stratum stays playable throughout: the deep strata keep their present themes until the later wave adds to them.

A stratum is Undermere's own data, a named range of floors, not an engine concept.

### The player sees them

- Crossing into a stratum logs a line of its own, "You pass beneath the kingdom.", before the floor's arrival line.
- The depth readout names the stratum beside the floor.
- The town's rumours and lore hint at what is deeper and older, so the direction is something a player learns before they walk it.

## The bestiary follows its themes

Monsters and mini-bosses move with the themes they belong to: the drowned dead and the Drowned Abbess go up to 3-4 with the cathedral, spiders and the Widow down to 5-6 with the warrens.
A load check holds it: every theme must have at least three favored kinds able to spawn in its window, so a theme is always told by what lives in it.

The difficulty curve is kept, not reshaped.
How strong a floor is stays as part 3 and part 4 tuned it, about seventy in a hundred and fifty delves won; only which kinds fill it changes.
A kind moved shallower gets a weaker early form, a drowned acolyte before the wight, and one moved deeper a stronger one, each a core and traits in the existing model.
The soak run by hand confirms the curve held, against its last numbers.

## The first wave

Four new themes, the early strata's: Old Town and the Harbour on floors 1-2, the Royal Wood and the Gardens and Menagerie on 3-4.
Each is a biome in `assets/biomes.ron` with its recipes in `assets/layout.ron`, its tiles, its decor and light, its favored kinds, its moods and its rumour, as every biome has.

Four landmarks, chosen so that between them they use every placement and every stairs rule the engine offers:

| Theme | Landmark | Placement | Stairs |
| --- | --- | --- | --- |
| Watch Barracks | the gatehouse: a curtain wall with its gate passage | Span | Beyond |
| Harbour | a sunken ship along the quay, half in deep water | Edge | Beyond |
| Gardens and Menagerie | the hedge maze, the way down at its heart | Center | Within |
| Royal Wood | the hunting lodge | Anywhere | Free |

A theme with a landmark gives every floor of it one; a theme without one grows as it does today.
The list grows over time: the reading dome, the great nave, the market square and the grand stair are the next candidates for the early strata.

## Order

1. In parallel: the engine's prefab facings; the engine's seeded first piece and the `landmark` pool kind; and Undermere's strata, the roster reordered, the bestiary moved, the curve soaked.
2. Then the first wave, its four themes and four landmarks, on the strata and the engine work.

## Not in this work

- Side branches and a forked way down: parked.
  When they come back, `dungeon::floor_of` is `Some` only for floors 1 to 10 and gates about fifteen systems (biomes, the pool plan, population, hidden things, finds, vault keys, loot, score), so a branch floor needs an effective depth; and the engine's `Ladder` is one line, so a fork is either the game's own `Transition` or a ladder that becomes a graph.
- A floor rule that forces a route decision.
- A rule that changes a whole floor.
- The later wave: Cisterns, Elder Wood and Barrows, and the deep strata's landmarks.
- Foundry.

## As built

Step 1, Undermere's strata, built on the `strata` branch.
Where it deviates from or extends the spec above:

- **Strata** are `examples/undermere/src/dungeon/strata.rs`: six rows (the thrones are the sixth), each a name, a floor range and the line said on crossing in.
  The crossing is said on the first arrival on any floor of a stratum when no other floor of it was visited, so a fall or a begun photograph still hears it once, ahead of the floor's line.
  Load checks hold the strata to a gapless cover of floors 1-10 and every biome to exactly one stratum.
- **The rail** names the depth on two rows under the gauges, `floor 3` and the stratum, as vitals facets; the vitals gained two rows.
- **Biomes**: Sunken Library and Drowned Cathedral on 3-4; Catacombs, Old Mines, Fungal Deeps and Spider Warrens on 5-6; Underlake alone on 7-8; Forge and Ashen on 9.
  Ambient light follows depth (22, 21 at 3-4; 18-20 at 5-6, the Warrens dark; 16 below).
  The Underlake's rumour is no longer told as a biome rumour, since its floors cannot draw another; the strata carry the hint instead, as four new rumours (strata two to five) told after the biomes.
- **Favored check**: `biomes::check_favored`, run in `Undermere::new`, demands three favored kinds with a spawn row on every floor of the biome's window, and refuses a name the bestiary lacks.
- **New kinds**: drowned pilgrim (early thrall), drowned acolyte (early wight, venomous, a touch that can freeze), pale scribe (early wraith, withering, a word that slows), drowned guard (the palace floors' brute, armored, on floor 4 as the ogre was; the early form of the throne's knights), myconid elder, giant spider, spiderling.
  The Drowned Abbess is softened (38 health, 1d8) for floors 3-4 and calls pilgrims, via a new optional `monster` argument of `CallDrowned`.
- **The curve, held**: the acolytes are the palace floors' killer, as the cave spider was before, fewer on floor 3 than on 4; the rats and jackals keep to the Drowned Town; the Underlake favors the will-o'-wisp over the black water rather than the troll, and its wight row is cut by a third since it now stands on both floors seven and eight; floor nine's troll, imp, salamander and golem rows are cut by about a third, since each of its biomes now favors three of them where it favored one; the spiders of floors 5-6 come a little less often.
- **Goblins** live with the mines: the war band comes only on a floor whose biome favors its leader (a new `themed` flag on `Band`), the Goblin Stockade stands only in the halls, cellars, barracks and mines, and the ogre-over-goblin feeding ground is gone.
- **Mini-bosses**: Abbess 3-4; Grubnak, the Widow and the Choirmaster 5-6; the Forgemaster unchanged at 7-9.
- Pool: flavor entries moved with their biomes; the Reliquary 3-8, Larder 5-6, Brood Chamber 5-8, Wagon 1-6.
- The save is version 11, refusing older runs; version 10 is main's own, the engine's saved actor.
- The rail's pinned rectangles move with the vitals' two new rows: the gear panel starts two rows lower and what is nearby is two rows shorter.
- **Seed one, which stalled**: the fingerprint's scripted adventurer struck at whatever stood beside it, and on seed one the strata put the run's shackled captive, a rat, beside its path on floor one.
  A blow at a captive is a walk into it, which begins the freeing, and the script's next intent, written while the player holds the turn for the work, left the freeing off at once, so it struck for good.
  The soak's script did the same, and all but three of the fourteen delves the soak counted as stalled on main were a captive in the way.
  Both scripts now leave the turn to the work while the adventurer is working, and the fingerprint strikes only a foe, by the bump's own rule; seed one is restored and the fingerprint re-baselined once.
  The rest were the soak waiting where nothing could be reached; on the one followed, a descent had set it down in a hidden room, which a descent may land in since a secret counts as found, and it never searched for the walled door; it now searches the walls of wherever it stands when the way on is out of reach.
  One more stall was a game fault, mended: a flooding vault kept wading-deep only the cell under someone standing in its far end, so whoever lingered by the far wall stood on an island of shallows in deep water for good; now none of the far end goes deep while anyone stands in it.

### The soak, 150 delves over 50 seeds

Main as it was is before the strata with the soak's old script, main with the scripts and the flood mended is the baseline the strata were tuned against, and the stalls are counted per plan, armor, weapon and staff.

| | won | died on floors 3, 4, 5, 6, 7, 8, 9, 10 | stalled |
| --- | --- | --- | --- |
| main as it was | 52 (35%) | 9, 14, 4, 1, 4, 9, 13, 30 | 4, 5, 5 |
| main, scripts mended | 68 (45%) | 6, 14, 6, 1, 7, 5, 16, 24 | 1, 1, 1 |
| strata, first tuning | 55 (37%) | 1, 2, 10, 5, 9, 8, 23, 21 | 5, 7, 4 |
| strata, as built | 70 (47%) | 6, 8, 11, 1, 6, 7, 11, 30 | 0, 0, 0 |

Main's three left were the soak waiting with nothing in reach, the stall the search now answers; main's soak ran before the search was written.
The win rate held, and so did the shape: floors 3-4 kill again, by the acolytes where it was the cave spider's venom, and floor 9 is back under its old share.
