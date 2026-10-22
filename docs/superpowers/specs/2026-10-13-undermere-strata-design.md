# Undermere: the strata and the first landmarks

Status: written 2026-10-13, from a design interview with the user; step 1, the strata, built on the branch `strata` (see "As built"); the first wave not yet built.
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
- **The rail** names the depth on one row under the gauges, `3 · The Palace and its Grounds`, and `Mereholt` in the town, so the row is always there; the gold is under it and the badges come last, by the engine's new `VitalsPanel::badges_last`, so neither the depth nor the gold moves when a status comes or goes or on going down the stair. The vitals gained one row; two cost what is nearby two of its rows, and the first review asked for one back.
- **Biomes**: Sunken Library and Drowned Cathedral on 3-4; Catacombs, Old Mines, Fungal Deeps and Spider Warrens on 5-6; Underlake alone on 7-8; Forge and Ashen on 9.
  Ambient light follows depth (22, 21 at 3-4; 18-20 at 5-6, the Warrens dark; 16 below).
  The Underlake's rumour is gone, since a biome's is told only when its floors could have drawn another and the Underlake stands alone on its floors; the strata carry the hint instead, as four new rumours (strata two to five) told after the biomes, and a biome added to floors seven and eight brings it back.
- **Favored check**: `biomes::check_favored`, run in `Undermere::new`, demands three favored kinds with a spawn row on every floor of the biome's window, and refuses a name the bestiary lacks.
  `effects::bodies::check_calls`, run when the library is read, refuses a call of the drowned naming a kind the bestiary lacks, which would otherwise load as a call that raises nothing.
- **New kinds**: drowned pilgrim (early thrall), drowned acolyte (early wight, venomous, a touch that can freeze), pale scribe (early wraith, withering, a word that slows), drowned guard (the palace floors' brute, armored, on floor 4 as the ogre was; the early form of the throne's knights), myconid elder, giant spider, spiderling.
  The Drowned Abbess is softened (38 health, 1d8) for floors 3-4 and calls pilgrims, via a new optional `monster` argument of `CallDrowned`.
- **The curve, held**: the acolytes are the palace floors' killer, as the cave spider was before, fewer on floor 3 than on 4; the rats and jackals keep to the Drowned Town; the Underlake favors the will-o'-wisp over the black water rather than the troll, and its wight row is cut by a third since it now stands on both floors seven and eight; floor nine's imp and salamander rows are cut by a third and its troll and golem rows by half, since each of its biomes now favors three of them where it favored one; the spiders of floors 5-6 come a little less often.
- **Goblins** raid out of the mines: the war band comes on floors five and six whatever biome was drawn, and the Goblin Stockade stands anywhere on floors two to six but the Catacombs, the Sunken Library and the Drowned Cathedral.
  The ogre over a goblin feeds on floors five and six beside the ghoul over a skeleton; a floor's feeder is the one its biome favors, the mines the ogre and the tombs the ghoul, and either where neither is favored, rather than the first row that fits.
- **Mini-bosses**: Abbess 3-4; Grubnak, the Widow and the Choirmaster 5-6; the Forgemaster unchanged at 7-9.
- Pool: flavor entries moved with their biomes; the Reliquary 3-8, Larder 5-6, Brood Chamber 5-8, Wagon 1-6.
- The save is version 11, refusing older runs; version 10 is main's own, the engine's saved actor.
- The rail's pinned rectangles move with the vitals' new row: the gear panel starts a row lower and what is nearby is a row shorter; the targeting box keeps its rows, its frame where the nearby list's rule was, so the list's heading stays readable over it.
- The spiderling is a pale `x`, so the three spiders of floors five and six, the giant `S`, the cave spider's violet `s` and the swarm, read apart at a glance.
- **Seed one, which stalled**: the fingerprint's scripted adventurer struck at whatever stood beside it, and on seed one the strata put the run's shackled captive, a rat, beside its path on floor one.
  A blow at a captive is a walk into it, which begins the freeing, and the script's next intent, written while the player holds the turn for the work, left the freeing off at once, so it struck for good.
  The soak's script did the same, and all but three of the fourteen delves the soak counted as stalled on main were a captive in the way.
  Both scripts now leave the turn to the work while the adventurer is working, and the fingerprint strikes only a foe, by the bump's own rule; seed one is restored and the fingerprint re-baselined once.
  The rest were the soak waiting where nothing could be reached; on the one followed, a descent had set it down in a hidden room, which a descent may land in since a secret counts as found, and it never searched for the walled door; it now searches the walls of wherever it stands when the way on is out of reach.
  One more stall was a game fault, mended: a flooding vault kept wading-deep only the cell under someone standing in its far end, so whoever lingered by the far wall stood on an island of shallows in deep water for good.
  Holding all of the far end shallow while anyone stood in it, the first mend, let a taker who lingered there loot it dry and a monster parked there hold the flood off for good; now the cell they stand on is held and one channel from it out of the far end, each step a step nearer the door, and the rest goes deep on time.
- **A human's walk into the captive being freed** left the work off and began it over at every press, the same loop the scripts hit, so a held key never freed anyone: the engine's `stop_work_on_key` now reads a direction key toward the work's own target as pressing on with it.
- **The review's retune**: with the war band and the ogre's feeding ground back on floors five and six, the acolytes come more often and in bigger groups on floor four and the drowned guard more often there, and on five and six the plain goblin, the cave spider, the spiderlings' group, the skeleton, the ghoul and the wraith come a little less often.
  The soak's script once threw darts for good at a sleeping wight it could see round a corner a bolt cannot turn; it now shoots only along the engine's own line of fire, and walks otherwise.
  The fingerprint is re-baselined once more, with seed four for seed two, whose script now comes to floor six without a kill.

### The soak, 150 delves over 50 seeds

Main as it was is before the strata with the soak's old script, main with the scripts and the flood mended is the baseline the strata were tuned against, and the stalls are counted per plan, armor, weapon and staff.

| | won | died on floors 3, 4, 5, 6, 7, 8, 9, 10 | stalled |
| --- | --- | --- | --- |
| main as it was | 52 (35%) | 9, 14, 4, 1, 4, 9, 13, 30 | 4, 5, 5 |
| main, scripts mended | 68 (45%) | 6, 14, 6, 1, 7, 5, 16, 24 | 1, 1, 1 |
| strata, first tuning | 55 (37%) | 1, 2, 10, 5, 9, 8, 23, 21 | 5, 7, 4 |
| strata, as built | 70 (47%) | 6, 8, 11, 1, 6, 7, 11, 30 | 0, 0, 0 |
| strata, review's changes before the retune | 58 (39%) | 5, 7, 7, 9, 7, 7, 19, 31 | 0, 0, 0 |
| strata, retuned | 68 (45%) | 2, 10, 12, 1, 5, 5, 13, 34 | 0, 0, 0 |
| strata, retuned, seeds 51 to 100 | 62 (41%) | 4, 11, 5, 5, 9, 5, 19, 30 | 0, 0, 0 |

Main's three left were the soak waiting with nothing in reach, the stall the search now answers; main's soak ran before the search was written.
The win rate held, and so did the shape: floors 3-4 kill again, by the acolytes where it was the cave spider's venom, and floor 9 is back under its old share.
After the review's retune floor four is again the deadliest of the early floors; floor five's twelve on the first fifty seeds was mostly noise, five on the next fifty, so over a hundred seeds the retuned curve is, per hundred and fifty delves, 65 won and 3, 10.5, 8.5, 3, 7, 5, 16, 32 died on floors three to ten, against main's 68 and 6, 14, 6, 1, 7, 5, 16, 24.
More delves reach the thrones than on main, and more die there; the thrones are unchanged.

### The first wave: the palace grounds

The palace half of the first wave, built on the `wave-palace` branch beside the town half on `wave-town`.
Where it deviates from or extends the spec above:

- **Two biomes on floors 3-4**, beside the library and the cathedral at weight one each, so a palace floor draws one of four.
  The **Royal Wood** grows caves among black trees on wandering one-wide game trails, by `hunting grounds` (clearings) or `deep wood` (glades round black pools, a ridge of fallen trees the wood bends round); its light is glowing moss, and its undergrowth is walked into at half a step more and seen into but not past.
  The **Palace Gardens** grow rectangles cut into hedges on two-wide gravel walks, by `parterres` (fountains standing in the parterres) or `menagerie` (small pens on the walks); its light is beds of pale blooms and the fountains' basins.
  Each has two moods, a rumour, two or three flavor dressings of its own in the pool and its decor.
- **A biome's own door**: a new optional `door` field in `assets/biomes.ron`, `door` if left out, which the wood hangs as undergrowth, so its trees have no doors in them; the load refuses a door no body can pass.
- **Favored kinds**: the wood favors the pale hound (packs, fast, keen-nosed), the white hart (fast, flees when hurt) and the will-o'-wisp; the gardens the hedge adder (venomous, unseen out of the light), the pale ape (heavy blows, sleeps deeply) and the monkey.
  The beasts come rarely outside their biomes, and the acolytes stay the palace floors' killer, favored in the library and the cathedral.
- **Landmarks**: the hunting lodge, `Seeded([Anywhere], Free)`, a timber hall with its kennels, bedroom and gear room grown out from three doorways; the hedge maze, `Seeded([Center], Within('>'))`, the way down at its heart and a dead end holding the remains of one who never found it.
  The pool gives floors 3-4 a required `landmark` budget of one and each landmark two to a run, so a run that draws one biome on both floors meets its landmark on each; a library or cathedral floor has none and grows as before.
  A load check, `biomes::landmark_problems`, holds every biome with a landmark to one on every floor of its window and enough to a run for all of them; the generator backstop asserts a `Within` landmark's way down stands on its mark.
- The potion of strength's weight drops from nine to eight: the new floors' finds put a run at five and a half against the four `finds.rs` holds it to.
- **Two tests that passed by how the streams fell**, found when the new content moved them: the confused goblin is set six steps off before each turn and stops at its first step astray, and the far door of the doors test is walled in.
- **A shackled captive** that takes things up no longer walks off its chain to fetch a blade lying near: its fetching is laid by hand before the hunt and anchored, as a pack's keeping and a flitter's flitting already were.

The soak, 150 delves over seeds 1 to 50 against main at `c1cf867e` (no game code changed since `fbbbbe5c`):

| | won | died on floors 3, 4, 5, 6, 7, 8, 9, 10 | stalled |
| --- | --- | --- | --- |
| main | 72 (48%) | 3, 4, 9, 1, 6, 7, 22, 24 | 0 |
| palace grounds | 70 (47%) | 5, 3, 10, 5, 6, 10, 12, 29 | 0 |

The curve held, so nothing was retuned.
Floors 3-4 kill eight against seven, but three of the eight are one seed whose adventurer turned a floor-three monster into a golem with an unknown wand of polymorph, which draws from the whole bestiary; without them the palace floors are a little softer, by the acolytes coming less often where the wood or the gardens is drawn, and still the acolytes and the pale scribe are what kill there.
The new beasts killed nobody.
