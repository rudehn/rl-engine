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
