# Grid Sage Games on map generation: a reference

Research notes compiled 2026-09-30 from the first-party writing of Josh Ge (Kyzrati), developer of Cogmind and POLYBOT-7, on the Grid Sage Games blog.
The purpose is to serve a later design session on procedural map generation for `examples/foundry`.
Everything in sections 1 to 9 is sourced, and each claim links the post it came from.
Section 10 is inference and is labeled as such.
Where a source does not say something, this file says "not stated" rather than guessing.

Conventions used here:

- A link such as [tunneling][tun] points at the post a claim came from; the full list with dates is in section 11.
- "Image:" lines give the image URL and a description of what it shows, written from looking at the downloaded image.
  Descriptions of what an image shows are my reading of the picture; what the caption says is attributed to the post.
- Images were not copied into the repo.

## 1. The approach as a whole

Cogmind does not have one map generator.
It has two layout generators (a "tunneler" room-and-corridor digger and a randomized cellular automaton for caves), two sources of handmade content (prefabs and "encounters"), and a per-map-type parameter file that says how much of each to use ([map composition][comp]).
He counts about two dozen map types, with the main complex made by the tunneler plus "a small amount" of handmade content, and branches needing more inputs ([map composition][comp]).

The pipeline for one map has three phases, in this order: A) layout, B) content (prefabs, machines, items and so on), C) player-affected content (allies, plot state, options) ([seed divergence][div]).
Layout is produced by a separate standalone program that knows nothing about game content, and it hands the game the layout plus recorded data about every junction, door, room and open area, and analysis such as a seclusion heatmap ([map composition][comp], [procedural layouts][lay]).
Validation happens at the end of the layout phase, before any content is loaded, so a rejected layout is cheap ([procedural layouts][lay]).

The handmade and the procedural are mixed at four different scales:

1. Whole static maps that are one giant prefab ([level design][lvl]).
2. Whole maps assembled from four large prefab quadrants (Garrisons, DSFs) ([garrisons][gar], [maps between maps][mbm]).
3. Large or important prefabs placed before generation, which the generator then grows around ("seeding") ([prefabs in depth][pre]).
4. Small prefabs dropped into rooms or cave walls after generation, chosen by a weighted encounter system ("embedding") ([prefabs in depth][pre], [caves][cav]).

Under all of that, ordinary rooms and halls that received no prefab are furnished by a data-driven "composition" system that picks machines and stockpiles by map theme ([furnishing][fur], [non-interactive machines][nim]).

The world is a spine of large, relatively predictable main maps with optional branches hanging off the sides, no backtracking, and a layout regenerated every run, so a single run cannot see everything ([world of robots][wor], [world map][wmap]).

The single design principle he repeats most often is loops: he tunes parameters for "the number and density of loops", because a layout without them forces backtracking, and "backtracking is not fun" ([procedural map generation][pmg]).
The second is that a map which looks good from above says little about how it plays from inside ([procedural map generation][pmg], [procedural layouts][lay]).

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2015/04/cogmind_map_type_composition.png>
A box diagram.
Left column has two groups: "Procedural Map Generation" (Tunneling Algorithm, Cellular Automata) and "Handmade Map Content" (Prefabs, with an arrow down to Encounters).
Right column is "World" (Main Complex, Branches).
A thick arrow runs from Tunneling Algorithm to Main Complex and a medium one to Branches; a thick arrow from Cellular Automata to Branches; thin arrows from Prefabs and Encounters to Main Complex; thick arrows from Prefabs and Encounters to Branches.
The caption says thicker arrows mean heavier emphasis ([map composition][comp]).

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2018/11/cogmind_mapgen_phases_major.png>
A flowchart: "A) Layout" then "B) Content" (Prefabs, Machines, Items, etc) then "C) Player-Affected Content" (Allies, Plot Stuff, Options, etc) ([seed divergence][div]).

## 2. Tunnelers: the room-and-corridor generator

### 2.1 What is stated

An empty (solid) map is seeded with one or more tunnelers.
They travel the map and open everything that becomes occupiable space: corridors, doors, rooms, junctions and halls ([tunneling][tun]).

The idea he credits is from the DungeonMaker project: tunnelers "can change their own parameters as they move", which is what keeps a large map from being repetitive ([tunneling][tun]).
The post's "source" link for the reference image is <http://dungeonmaker.sourceforge.net/DM2_Manual/manual1.html> ([procedural map generation][pmg]).
That manual is third-party and was not read for this file.
He lists what he disliked in that algorithm: useless and redundant corridors, and only odd-width corridors ([tunneling][tun]).

Parameters he names for a tunneler: width, direction, speed, chance to turn, chance to create rooms, size and shape of rooms to create, how much space (padding) to leave between itself and other dungeon objects, and when to quit ([tunneling][tun]).
"A single algorithm is capable of producing a broad range of unique layouts by adjusting dozens of parameters" ([map composition][comp]).
Numeric values for these parameters are not stated in any post read.

What a tunneler does as it goes:

- It tries to dig rooms off its edges "when possible".
  Rooms are places to hide, find parts, or hold machines.
  A room with more than one door is "usually an intersection of sorts" giving access beyond its own corridor ([tunneling][tun]).
- It sometimes builds a junction (a small square node) when it turns or changes width.
  Junctions on main corridors "will almost certainly" get a terminal ([tunneling][tun]).
- Tunnelers "spill into and contribute to" larger open areas called halls.
  Halls usually get larger machines that subdivide them and give cover ([tunneling][tun]).
- Tunnels shrink and grow two cells at a time, so an even-width tunneler ends up odd-width after shrinking to 1 and growing again ([tunneling][tun]).

Corridor width reasoning:

- 2-wide is narrow but lets units pass each other.
- 3-wide is "probably better as a standard" for a game fought at range, because a firing line needs room.
- Even widths give off-center connections that look worse, so he "may not bother" with them ([tunneling][tun]).
- Wider corridors and bigger rooms in general follow from ranged combat; melee-heavy roguelikes have small rooms and narrow corridors ([procedural map generation][pmg]).
- He says fights are typically at "an average of 15 or more cells away" ([tunneling][tun]).

Padding reasoning: more padding between corridors and rooms makes it harder to blast through into a neighbor.
He keeps padding low on purpose, because destructible terrain is only interesting if a wall is thin enough to use ([tunneling][tun]).

Hidden doors and hidden corridors are a generator feature.
They "are also usually the only way the map generator will directly connect rooms with one another" ([procedural layouts][lay]).
In the main complex their purpose is to give hostiles extra routes and ambush options, not to hide loot, and they sit in the back or side walls of rooms ([garrisons][gar]).

When several tunnelers build one map, the generator must confirm afterward that they met and joined their creations ([dungeon metrics][met]).

### 2.2 Seeding tunnelers from a layout plan

This is the part most relevant to "not predictable", and it is described in [procedural layouts][lay].

Before any parameters are set, he draws "orientation mockups" for the map type: where each guaranteed entrance or exit goes, and which way its tunneler may head.
Tunnelers start at the points of interest and dig outward, "working backwards from all the points of interest until they all collide".
Because of their distance apart and the way they travel, "they're not likely to directly connect with one another for a while, producing circuitous routes".

The elements of a plan:

- Guaranteed entrance or exit: placed "randomly shifted around in the approximate area indicated", with a tunneler digging "in a random direction from among those represented by the lines".
- Non-access tunnelers: directional tunnelers that pick a random direction from their options and exist "just to fill space rather than act as access points".
- Random barriers: the generator "may also decide to randomly instate one or more optional barriers" that tunnelers and rooms cannot cross, which makes the local area more circuitous.
- Fixed barriers: always present, to enforce a specific overall shape.
- Reserved blocks: space for random placement of prefabs that lead to branches.

A map type has several such patterns and each generation picks one.
His reason: the patterns are different enough that "the player doesn't know which pattern they're currently playing in (or even that there's a pattern at all!) and can't reliably deduce a way out without more info" ([procedural layouts][lay]).
He also says that in the end the plans "are all represented by numbers in Cogmind's mapgen scripts" ([procedural layouts][lay]).

Main exits are deliberately far away and "built into the very foundation of the map's layout"; branch exits are "placed more randomly", so a player is more likely to find a sideways exit before an upward one ([procedural layouts][lay]).
Players do learn that map edges are more likely to hold main exits ([procedural layouts][lay]).

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2019/02/cogmind_materials_mapgen_layout_mockups.png>
Three square plans for the Materials map, labeled "cross", "spiral" and "corridors", with a legend: "entrance/exit tunneler w/dir" (yellow), "non-access tunneler w/dir" (gray), "random barrier" (dark gray), "possible prefab exit area: mine" (brown).
Cross: four yellow tunneler groups at the middle of each edge (top, left, right, bottom), a brown block in the center and one in each of the four corners (the corner blocks also carry gray non-access tunneler marks).
Spiral: yellow tunneler groups near the four corners, four dark barrier bars arranged as a pinwheel (one from the left edge, one down from the top, one up from the bottom, one from the right edge, each stopping short of the middle), and one brown block in the center.
Corridors: one yellow group at top center and three along the bottom, two light gray fixed walls rising from the bottom edge to about mid-height that split the lower half into three lanes, and brown blocks in the top-left and top-right corners.
Each yellow group is drawn as several parallel strokes pointing in the allowed directions.

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2019/02/cogmind_mapgen_layout_mockups_various.png>
Five more plans for other map types, of different aspect ratios (three square, one wide rectangle, one small square).
They use the same vocabulary plus large solid cyan and red rectangles (big reserved regions, meaning not stated), dashed barrier lines with gaps, and dense yellow fields where a whole band or quadrant is a tunneler start region.
The point visible from the picture is that plans differ a lot between map types: one is two horizontal yellow bands above and below a central cyan strip, another is a left half of tunnelers facing a right half that is one cyan block.

The same data appears as a text table in [prefabs in depth][pre], covered in section 5.2.

### 2.3 What tunneler maps look like

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2014/06/cogmind_map_main.png>
A tall map (the image is 400x800 pixels; cell scale not stated) with EXIT at top center and ENTRANCE at bottom center.
White corridors of clearly different widths, from 1 cell to about 6 cells, with the widest forming a few long east-west and north-south runs.
Dark gray rectangular rooms are packed along both sides of nearly every corridor, mostly near-square or about 2:1, each with one small red door mark on the corridor side.
Light gray squares (junctions) sit at corridor turns and width changes.
A few irregular white blobs (halls) appear where corridors merge.
The corridor network has many closed loops; almost no corridor ends without a room.

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2014/06/cogmand_map_tunnelers_huge.png>
Captioned "the largest likely map" (200x200, [tunneling][tun]).
About a hundred rooms, long corridors that run half the map before turning, several very wide corridors, large irregular halls with ragged edges in the lower third, and sizable untouched black regions between clusters.
Thin 1-wide corridors wander around the periphery linking small junctions.
He notes the algorithm still needed cleanup "to reduce overall jaggedness" at this stage ([tunneling][tun]).

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2014/06/cogmind_map_tunnelers_even.png>
One tunneler starting top center.
Its first run of even-width corridors is orange and later odd-width corridors are blue, showing the width drift described above ([tunneling][tun]).

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2014/06/cogmind_map_tunnelers_padded.png>
The same generator with more padding: rooms stand apart from corridors and from each other, with visible black margins, and the map reads as sparser ([tunneling][tun]).

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2014/06/cogmind_map_tunnelers_junctions.png>
Junctions highlighted in red: about twenty squares of 2x2 to 5x5 cells at corridor intersections and bends, spread across the whole map rather than clustered ([tunneling][tun]).

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2014/06/cogmand_map_tunnelers_halls.png>
Halls highlighted in orange: roughly a dozen rectangular open areas, larger than rooms (some four or five room-widths long), mostly along a diagonal band, each bordered by corridors on more than one side ([tunneling][tun]).

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2015/04/cogmind_mapgen_variety.gif>
Ten frames, each a composite of several main-complex layouts of different sizes side by side.
Common features across frames: prefab areas outlined in orange at edges or centers, cyan segments marking hidden corridors between adjacent rooms, and in some frames a large empty black region in the middle of a map with the rooms forming a ring around it.
His comment is that the differences between main-complex maps are intentionally subtle because they are parts of one complex ([map composition][comp]).

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2019/02/cogmind_mapgen_visualization_hidden_corridors.png>
A large map with hidden doors and corridors in cyan: about twenty short segments, each one to six cells long, each joining two rooms that are close together, sometimes with an L bend.
Two orange-outlined prefab rooms sit at the top-right corner and bottom-left.
The middle of the map is an empty black region, so the playable space is a ring ([procedural layouts][lay]).

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2014/10/cogmind_factory_evolution.png>
The 2012 7DRL factory floor above the 2014 one.
The newer map is about twice the side length and its rooms are visibly filled with arrays of machines, where the old one has empty rooms ([world of robots][wor]).

### 2.4 What is not stated about tunnelers

The posts do not give: the per-step procedure of a tunneler, the probabilities for turning, branching or spawning rooms, how child tunnelers are created, room size ranges, how doors are positioned on a room wall, how halls are formed or detected, how junction size is chosen, or the rule that ends a tunneler.
He says of the layout process that "there's a lot more to the process for tunneler-created maps, but beyond this point it gets extremely Cogmind-specific" and stops there ([procedural layouts][lay]).
A reimplementation has to design those steps; the DungeonMaker manual linked above is the stated ancestor.

## 3. Caves: randomized cellular automata

### 3.1 The algorithm

He does not use the standard whole-grid automaton.
He uses a variant credited to Andy "Evil Scientist" Stobirski: the adjacency rules are the usual open/close rules, but "instead of repeatedly applying the rules across the entire map at once you pick cells at random" ([cellular automata][ca]).
The extra knob this gives is the number of cells randomly visited.
Fewer visits give rougher maps, and a final smoothing pass still makes them usable.
He likes that random application gives variety inside one map: "a mix of sharp and round, large and small areas" ([cellular automata][ca]).
The exact neighbor thresholds and visit counts are not stated.

### 3.2 Connectivity by tunnels, on purpose

The automaton is allowed to leave many disconnected caves.
Rather than deleting the unconnected ones or growing them together, he digs tunnels between them, for two reasons: it fits the theme, and it leaves "many smaller caves that can be individually identified as rooms for purposes of object placement" ([cellular automata][ca]).
Caves are found by flood fill before they are connected ([dungeon metrics][met]).

His connector differs from the original in that it: removes most pointless U-bends, runs multiple corridor-digging phases that may not all apply to every cave, has an adjustable ratio of cave-spawned corridors to branching corridors, and sometimes digs wider tunnels based on parameters and on the relative size of the caves being joined ([cellular automata][ca]).

### 3.3 Guided generation

A square cave map in this disconnected style is not fun, because cave edges are full of nooks that must be checked up close, unlike a rectangular room that can be scanned at a glance ([cellular automata][ca]).
The fix is "guided" generation: let the automaton run, but force the cave to fit a general shape, such as a narrow band, so that the map keeps moving in one direction ([cellular automata][ca]).
The real cave maps have "a relatively linear overarching flow structure" with fewer loops than other areas, enough to avoid forced backtracking ([caves][cav]).

### 3.4 Mines and composites

Mines are a variant: small square maps with rectangular dug-out rooms for machinery and storage joined to cave pockets by corridors, with "a higher number of connections and loops" ([cellular automata][ca], [caves][cav]).
One map may also combine both generators, such as a tunneler-built base inside a cave system ([cellular automata][ca]).

### 3.5 Images

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2014/06/cogmind_map_automata_tunnels.png>
Individual cave blobs joined by 1-wide straight and L-shaped tunnels ([cellular automata][ca]).

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2014/06/cogmind_map_automata_guided.png>
A tall 1:2 map where the caves form one sinuous band from top-left to bottom-right, spiky with short offshoots, with a wider chamber with a few pillars in the middle ([cellular automata][ca]).

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2014/06/cogmind_map_automata_mine.png>
A small square map: three clean rectangles (one tall, one central, one small at top) among rounded cave pockets, all joined by 1-wide orthogonal corridors that form several loops ([cellular automata][ca]).

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2014/06/cogmind_map_composite.png>
A large square cave map with a compact room-and-corridor complex of about a dozen gray rooms in the upper middle, captioned "A hidden base?" ([cellular automata][ca]).

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2016/03/cogmind_cavegen_analyzed.png>
An in-game full cave map, wide (roughly 2:1), with two inset overlays: "MAP FLOW", a red polyline from ENTRANCE at the upper right to EXIT at the left with a few side spurs, and "MAP LOOPS", about ten blue ellipses of varying size marking loopable routes ([caves][cav]).

## 4. Composition requirements and dungeon metrics

### 4.1 Reject and retry

The generator is allowed to fail.
Every finished layout is tested against requirements and thrown out if it misses any ([dungeon metrics][met], [procedural layouts][lay]).
"Some Cogmind maps might fail dozens of times before loading", and this is not noticeable because the checks run right after layout, before content ([procedural layouts][lay]).
Elsewhere he says some maps "may occasionally take up to 2~4 seconds to load due to throwing out a large number of invalid layouts" ([desire paths][des]).
If a parameter set makes valid maps too rare, that is a signal to change the parameters ([procedural layouts][lay]).

Requirements he lists, each "tested for explicitly on completion" ([procedural layouts][lay], [dungeon metrics][met]):

- Openness: the share of cells that are occupiable, with a lower and an upper bound.
  The example values given are 15~30% and 40~60% for two caves from the same algorithm ([dungeon metrics][met]).
- Room counts by size class.
  The example values are "20 or more small rooms" versus "at least 20 medium and 5 large" ([dungeon metrics][met]).
- Consistency of room sizes (mostly small with a few massive, or all similar) ([procedural layouts][lay]).
- Open area dimensions: a cap on how large a hall formed by meeting corridors may get ([procedural layouts][lay]).
- How many such open areas there are, since their ratio to rooms and corridors changes play ([procedural layouts][lay]).
- The prevalence of hidden corridors ([procedural layouts][lay]).
- Reachability of entrances and exits, by pathfinding ([procedural layouts][lay], [dungeon metrics][met]).
- A minimum path distance between entrances and exits, because an exit near the entrance leaves most of the map unexplored ([dungeon metrics][met]).
- Summary statistics may also be requirements: percent composition by cell type, maximum and average seclusion, the largest single hall or cave ([dungeon metrics][met]).

The actual thresholds per Cogmind map type are not stated.

An alternative he mentions and does not use: generate many maps and hand-pick a fixed set, which limits the pool to what the developer chose ([dungeon metrics][met]).

### 4.2 What gets recorded

Rooms and corridors dug by tunnelers are recorded as they are dug.
Caves are found afterward by flood fill.
Also recorded: doors and the direction each faces, which rooms and corridors attach to each junction, which tunnels link which caves, and which caves are directly reachable from each other ([dungeon metrics][met]).

Junctions get a rating "based on their size and number of corridors and rooms they link to", used to post sentries and, later, heavier guards at the highest-value junctions ([desire paths][des], [area denial][den]).

### 4.3 Seclusion (tunneler maps)

Every room is rated by how far it is from the fastest routes across the map.
The shortest paths between all entrances and exits are computed; then each room's doorway pathfinds to the closest point on any of those paths; the distance is the seclusion ([dungeon metrics][met]).
In the tool these routes are drawn as dots along the shortest paths "because referencing complete lines is both less clear and too CPU-intensive" ([procedural layouts][lay]).
Use: "the more secluded a room, the more likely it will hold something of value", and individual higher-rated items prefer more secluded places ([dungeon metrics][met], [furnishing][fur]).

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2014/06/map_metric_seclusion.png>
A square map with four pink access points (top, left, right, bottom-center) joined by green shortest paths that mostly follow the wide corridors.
Rooms far from any green line are red, darker with distance: the whole bottom strip, the bottom-left corner and the upper right are red, and rooms along the green lines are plain gray.
About a third of rooms are shaded.

### 4.4 Connectedness (cave maps)

For caves he uses a different measure: a cave's connectedness is "the number of immediately connected caves plus the number of caves those are connected to as well" ([dungeon metrics][met]).
Low-connectedness caves in different parts of the map are candidates for entrances and exits, and others for loot, secrets or nasty surprises.
Well-connected caves are where enemy activity concentrates ([dungeon metrics][met]).

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2014/06/map_metric_connectedness.png>
A guided cave band with each cave tinted red by connectedness; the caves along the central spine are bright and the dead-end caves at the tips are dark.

### 4.5 Tooling

- Always output the seed of every generated map, so a bad one can be regenerated and stepped through ([dungeon metrics][met], [seeds][seed]).
- The layout tool has view modes: typed (rooms, junctions, halls, tunnels in different colors), monochrome open versus closed, paths and seclusion, halls, hidden corridors ([dungeon metrics][met], [procedural layouts][lay]).
- Parameters live in a text file; change it, press a key, regenerate ([map composition][comp]).
- He watches repeated generations for about 30 minutes per map type looking for outliers, then tunes ([procedural layouts][lay]).
- In game, a debug overlay shows where encounters landed, colored by category ([map composition][comp]).

## 5. Prefabs

### 5.1 Authoring and file format

Prefabs are drawn in REXPaint as multi-layer images (`.xp`) with a companion text definition file ([dungeon prefabs][dp], [prefabs in depth][pre]).

Layers:

- Layer 1: terrain, one palette color per cell type.
- Layer 2: linking and integration information, such as a numbered cell where a tunneler starts.
- Layer 3: machines and props, drawn as line art.
- Layer 4: reference characters for objects, always drawn in green ([dungeon prefabs][dp], [prefabs in depth][pre]).

His conventions for layer 4: upper-case letters for props and traps, lower-case letters for entities, digits for items, punctuation for debris ([prefabs in depth][pre]).

The definition file is a list of lines, one per reference character.
A definition file may be shared by several prefab images with similar contents ([prefabs in depth][pre]).
Each line has a character, a `[TYPE]` and a `[TAG]`, then optional keywords.
The five types are PROP, TRAP, ENTITY, ITEM and DEBRIS.

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2017/01/cogmind_prefab_definition_object_types.png>
Five example lines: `A [TYPE]PROP [TAG]"prop tag/name here"`, `B [TYPE]TRAP [TAG]"..."`, `a [TYPE]ENTITY [TAG]"..."`, `1 [TYPE]ITEM [TAG]"..."`, and a backtick line `[TYPE]DEBRIS [TAG]DEBRIS`, each "followed by any number of relevant keywords".

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2017/01/cogmind_prefab_definition_keyword_samples.png>
A commented list of keywords.
General: `[SA]` names a "Special Ability", which is a script; `[TRAIT]` attaches a string that scripts can test.
Props: `[FACING]` for a multi-cell prop's orientation; `[INTERACTIVE]DISABLE` and `[INTERACTIVE]CUSTOM`; `[HACK]"..."` adds a specific hack to a terminal.
Traps: `[TARGET]`, `[ARRAY]n` to group traps, `[KNOWN]1`.
Entities: `[FACTION]`, `[LEADER]1`, `[PARTY]GUARD`, `[AI_MOVE]WANDER`, `[AI_COMBAT]FLEE`, `[WANDER_SHIFT](10,10)`, `[INVENTORY]"Hvy. Assault Rifle(2)|[Autogun|Minigun]"`, `[DISABLE]` with BROKEN, UNPOWERED, DORMANT, REWIRABLE or a turn count, `[INTEGRITY]20-70`, `[PART_INTEGRITY]80-90`.
Items: `[INTEGRITY]20-70`, `[FILL]1`, `[AMOUNT]200`, and `[SINGLE_INSTANCE]1`, which means that once this item is placed, it "can never be placed anywhere else on the same map, even by other prefabs".
Debris: `[DENSITY]50-100`.

Any reference may also take `[SHIFT]` with (+/-x, +/-y), which moves the object randomly within that area so it is not always in the same cell ([prefabs in depth][pre]).

He says the whole prefab system is "a combination of external text files and binary image files, processed by about 3,000 lines of code" ([prefabs in depth][pre]).

### 5.2 Seeding: prefabs placed before generation

"Step 1 in the map generator is to go down a list of potential prefabs and configurations and put them on the map" ([prefabs in depth][pre]).
This is for prefabs that define the feel or difficulty of the whole map: large ones, and entrances and exits, because placing them first makes it easy to control the distance between them ([prefabs in depth][pre]).
Other candidates are "huge semi-static areas, sometimes as large as 25x25 or even 100x100" ([prefabs in depth][pre]).

How a seeded prefab joins the map: the cells around it are marked off limits so the generator cannot encroach, and a marked cell on its edge starts a tunneler of a stated width, heading away, which "will eventually connect with the rest of the map" ([dungeon prefabs][dp]).
Edges may also be left as "tunnellable earth", so later corridors may happen to link there, but only the tunneler exit is guaranteed ([prefabs in depth][pre]).
In caves, seeded prefabs use direct links instead of tunnelers "so they don't get out of control and take over the caves" ([dungeon prefabs][dp]).
Features support randomized position, so the same point of interest is not always in the same part of the map ([dungeon prefabs][dp]).
Maps also support features that are not prefabs, most usefully areas declared off limits to the generator ([dungeon prefabs][dp]).

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2017/01/cogmind_materials_mapgen_static_features.png>
The complete static feature list for Materials maps, a text table with columns: variant, random, type, shared prefab info, sub-variant, prefab facings, block spawns, and area x, y, w, h.
Variant 1 has six `[FEATURE]` rows, all with random -1 and block spawns ALL: `MAT_00_NS_MIN` and `MAT_00_EW_MIN` at (45,45) size 9x9, sub-variant 1, facings NESW; and four `MAT_00_S_MIN` rows of size 9x10 with sub-variants 2 to 5 at x 4-10 or 78-88 and y 4-10 or 78-88, the two southern ones with facing N.
Variant 2 has four `BLOCKED` rows with random 50 at (20,66) 1x33, (0,20) 33x1, (80,0) 1x33 and (66,80) 33x1, plus the two center prefabs at (45,45).
Variant 3 has two `BLOCKED` rows with no random value at (33,50) 1x50 and (66,50) 1x50, plus two `MAT_00_S_MIN` at x 4-10, y 4-10 and x 78-88, y 4-10.
The post explains variant 3: two barriers that stop pathways linking across them, two top-side mine-entrance prefabs to the west and east, which block all entity spawning in their area and have slightly variable coordinate offsets ([prefabs in depth][pre]).
The meaning of the "random" and "sub-variant" columns is not stated in the text.
My reading, not his: the coordinate ranges are the random offsets, random 50 is a 50 percent chance per barrier, and the three variants are the cross, spiral and corridors plans of section 2.2 written as numbers.

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2017/01/cogmind_materials_mapgen_mines_entrance_prefab.png>
The mine entrance prefab in REXPaint: a small room about 9 wide, its floor ringed on left, right and bottom by brown "tunnellable earth" cells, a machine across the top marked with green `A`s, a red item reference `4` in the middle, and a yellow `3` at the bottom edge, which is the 3-wide tunneler start heading south.

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2017/01/cogmind_materials_mapgen_mines_entrance_prefab_data.png>
Its two-line definition: `A [TYPE]PROP [TAG]"GENERIC_MACHINE_"` and `1 [TYPE]ITEM [TAG]"Mining Laser"|"Med. Matter Pod" [AMOUNT]1`.

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2017/01/cogmind_materials_mapgen_variant_3_final.png>
The finished variant 3 map: the two mine entrances in the top corners, two thin vertical barrier lines in the lower half at about one third and two thirds of the width, and rooms and corridors filling three lanes below and a connected band above.
No corridor crosses a barrier line.

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2017/01/cogmind_mapgen_large_prefabs.png>
Part of a map with two large seeded prefabs outlined in orange, each about three ordinary rooms wide, one with internal partition walls, both attached to wide corridors and surrounded by ordinary rooms.

### 5.3 Embedding: prefabs placed after generation, chosen as encounters

"Most of the prefabs in Cogmind are not applied until after the initial phases of map generation are complete" ([prefabs in depth][pre]).
Which prefabs go in is decided by the encounter system (section 6).
The generator's record of every room and its doors is what makes this possible ([prefabs in depth][pre]).

Three room-based varieties are described.

Enclosed room (the most common), steps as given in [prefabs in depth][pre]:

1. Select a random available room and compare it against the sizes of the prefab alternatives for the current encounter.
   One encounter may have several prefabs of different dimensions; any that fit are candidates and oversized ones are ignored.
   The prefab may be rotated to fit, and may be flipped horizontally at random.
2. Find a target rectangle bordering the doorway.
   Prefabs are always drawn facing south.
   The prefab is rotated so its bottom edge is on the door side, and its position along that wall is random among those that keep the door on the prefab's bottom edge.
3. The room is usually larger than the prefab, so the surplus is filled back in with earth and walls are rebuilt along the new inner edge.
   This resizing is optional; the surplus can be left as open space.
4. Place the prefab terrain and its objects.

Post-placement modifications: the door may be widened to a multi-cell door to signal a special room from outside, or removed to leave an open entry, or the door and the entire front wall may be removed to make a "cubby" open to the corridor ([prefabs in depth][pre]).
Storage depots on the earliest floors use the cubby form so they can be spotted from afar ([prefabs in depth][pre]).

Accessible room, steps as given in [prefabs in depth][pre]:

1. Find a room large enough; any number of doors is fine.
2. Position the target rectangle anywhere in the room at random.
3. Place the prefab and objects; the room's structure is not changed.

These prefabs have no internal walls and must leave all their edges walkable, because the room may have several entries and an obstruction could illogically block one.
They exist because multi-door rooms need content too ([prefabs in depth][pre]).

Drawbacks he states: with encounters choosing random rooms there is no control over where encounters sit relative to each other, and the packing is suboptimal, though "the player won't notice any of this" ([prefabs in depth][pre]).
He chose encounter-first over room-first because encounters are what matter for balance ([prefabs in depth][pre]).

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2017/01/cogmind_room_prefab_embedded_process.png>
Four numbered panels plus a rotated example.
Panel 1: an empty room about 14 by 8 above a corridor, with one orange door in its bottom wall near the left.
Panel 2: an olive rectangle about 9 by 6, the prefab's footprint, placed inside the room against the bottom wall and overlapping the door.
Panel 3: everything in the room outside that rectangle refilled with earth, and new wall drawn around the rectangle, so the room is now the prefab's size.
Panel 4: the prefab's machines and an entity placed inside.
The right column repeats this for a room whose door is on its west wall, with the prefab rotated 90 degrees so that its bottom edge faces west.

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2017/01/cogmind_room_prefab_embedded_process_modifications.png>
The same room twice: once with the single door removed leaving a gap, once with a four-cell-wide door.

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2017/01/cogmind_room_prefab_embedded_process_cubby.png>
The same room with its entire front wall removed, open to the corridor.

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2017/01/cogmind_room_prefab_embedded_process_rotated.png>
An accessible-room example: an irregular room with three doors on different walls, a small olive rectangle placed away from the walls, and a small machine cluster placed there with the room otherwise untouched.

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2017/01/cogmind_mapgen.png>
A finished layout with rooms colored by a debug category (red, orange, yellow, cream; the legend is not given in the post): roughly half the rooms orange, a quarter red, small ones yellow, three large cream halls.

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2017/01/cogmind_tunnel_map_encounters.png>
An in-game map with encounter rooms overlaid in solid color by category: about ten green, four orange and four red rectangles scattered evenly over a map of about sixty rooms, the rest left dark and filled only with machine arrays.
So roughly a quarter to a third of rooms hold an encounter in this sample (my count from the image).

### 5.4 Embedding in caves

Two methods, both in [caves][cav].

Centered: the prefab is dropped on the geometric center of its parent cave, meaning a point that gravitates to the cave's largest open area, not the center of its bounding box.
It overwrites both open and closed cells under it and includes an open buffer around itself.
It may overlap earth and its own cave, but not other caves.
Centered prefabs must be rectangular.

Embedded in a wall, the eight steps as given:

1. Pick a random prefab for the encounter and rotate it to a random cardinal direction.
2. Pick a random open cell in the cave as one front corner; from the length of the front face find the other front corner; if that is not inside the cave, try another point.
3. Measure along the two imaginary sides of the prefab until each hits a wall.
4. Each prefab has a hand-set `maxProtrusion`, the most its depth may stick out into the cave.
   If either measured distance exceeds it, push both corners back toward the wall until both are within it; if that is impossible, return to step 2.
5. Validate: the outer rectangle must contain no stairs, and the inner rectangle must be entirely earth, not even open cells of the same cave, because that would create unplanned entrances.
6. Require at least a 1-cell wall of earth around the sides and back of the inner rectangle.
7. Place the prefab, then convert to wall any earth on the outer edge adjacent to open prefab space.
8. Add objects from the definition.

Retry policy: if no valid area is found in this cave within a threshold of tries, try other prefabs of the same encounter; if that fails, start over with another cave.
"Encounters are chosen first, so the goal is to find a valid place for that encounter" ([caves][cav]).

To make room for embedding without loosening the caves themselves, he widened the band of diggable earth around the outer edges of the map, after rejecting a two-phase scheme that reserved earth pockets between caves because it lengthened corridors everywhere ([caves][cav]).
Embedded prefabs may be non-rectangular because their sides and back are guaranteed to be earth ([caves][cav]).

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2016/03/cogmind_caves_prefab_embedded_process.png>
Eight numbered panels matching the steps: the prefab footprint, two gray corner markers on the cave floor, green side lines measured up to the cave wall, the lines shortened by maxProtrusion, the target rectangle shaded with a dark outer and bright inner region, the alcove dug, and finally a terminal, a door and item markers inside.

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2016/03/cogmind_caves_prefab_centered.png>
A rectangular walled outpost placed in the middle of a large cave with an open margin all around it ([caves][cav]).

### 5.5 Maps made entirely of prefabs

Garrisons: the map is divided into quarters; each quarter picks a random prefab and rotates it to fit, so the four corners meet to form a central hub ([garrisons][gar], [garrisons 2.0][gar2]).
Rules each quarter prefab follows: one primary arm from the hub to that quarter's access point; the widest corridors run hub to access point, so navigation is easy until the player leaves the main path; most quarters hold one relay guarded by a checkpoint ([garrisons][gar]).
There were 44 quadrant variants, later 56 ([garrisons 2.0][gar2]).
Quarter types are weighted, with "the more interesting and/or rewarding quarters" rarer ([garrisons][gar]).
Making the first several dozen took "about a day", against the larger cost of writing a generator ([garrisons][gar]).
The stated price: players learn to recognize areas, and after about ten Garrisons in one run "some of these corridors start looking all too familiar" ([garrisons][gar], [garrisons 2.0][gar2]).

Garrisons 2.0 fixed that by adding embedded encounters to prefab-only maps.
He hand-outlined "optional placeholder rooms" in every quadrant; the encounter system treats them as empty rooms, and any placeholder not used "would actually not appear in the final map" ([garrisons 2.0][gar2]).
Embedding steps there: find a placeholder large enough; choose one of its drawn connection points at random; shift the prefab against that edge with a random offset along it that still leaves the interior reachable; fill unused room area with earth; place contents and any needed walls ([garrisons 2.0][gar2]).
Each encounter also states the form of its connection: open, hidden door or phase wall, either forced or weighted ([garrisons 2.0][gar2]).
Each map type sets what percentage of available encounter rooms to try to fill: Garrisons 50%, some maps 100% (caves, with many fluff encounters), Access 10% ([garrisons 2.0][gar2]).

DSFs are also four quadrant prefabs, with no variants: each quadrant is present or absent, and at least two must be present, one for the entrance and one for the exit ([maps between maps][mbm]).

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2015/10/cogmind_garrison_layout_labeled.png>
A square map with a central hub of thick white corridors in a pinwheel, four green access points near the corners at the ends of the wide arms, red checkpoints lining the hub and arms, yellow relays on side spurs, one blue generator in a room in the top right, and thin gray side passages branching off.

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2022/07/cogmind_garrison_prefab_quadrant_rexpaint_beta12.png>
One quadrant in REXPaint: black corridors, green object references, and about six brown-outlined rectangles of very different sizes (one tall strip down the left side, small boxes near corridors) which are the placeholder rooms, each joined to a corridor by a short brown stub.

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2022/07/cogmind_garrison_mapgen_with_encounter_possibilities_1.png>
A whole Garrison in the map generator: the white and gray corridor skeleton in the middle and about twenty dark gray placeholder rectangles around the outside, most of the map's area.

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2022/07/cogmind_garrison_prefab_embedded_process.png>
The four embedding steps, shown twice (linking from the south, and rotated to link from the west): the empty placeholder, the footprint pushed against the chosen edge, the rest refilled, and the contents placed with a short entry stub.

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2021/12/cogmind_DSF_annotated.png>
A four-quadrant DSF, labeled: a heavy defender in the center, four "guaranteed authchips" boxes around it, a "categorical item cache (quadrant)", a "hidden superior item cache (guarded)", a "dormant corner specialist", a "matter repository" in a corner, and "exit in corner opposite from entrance".

### 5.6 Why prefabs, and how many

Reasons given: prefabs restore character that an algorithm cannot produce outside its parameters; they stand out and are memorable; they give players common ground to discuss; and "I don't want to rely on finding the right place in a map for a certain encounter to occur. I'll just build it" ([dungeon prefabs][dp]).
"Even procedural maps get boring once players start to see through their patterns" ([prefabs in depth][pre]).
Variable content inside a prefab is "a relatively cheap way to make even handcrafted content stay fresh despite repeated exposure" ([prefabs in depth][pre]).

Counts as of January 2017 ([prefabs in depth][pre]):

- 148 base encounter types, 115 of which use prefabs.
- 372 prefabs in all.
- Largest 100x90, smallest 3x2, average 7x7.
- The encounter with the most alternatives has 12 prefabs.

## 6. Encounters: what goes in which room

### 6.1 Definition

An encounter is handmade content: "loot, enemies, friends, or basically anything, sometimes combined with procedural elements, and sometimes but not always associated with one or more prefabs" ([prefabs in depth][pre]).
It can be as small as a local patrol or a stockpile, or a scripted story event ([map composition][comp]).
Encounters are an extra layer on tunneler maps, and the only way content gets into cave maps, because caves are more linear and cannot use the same distribution system ([map composition][comp]).

### 6.2 Selection rules

Each map type has its own pool of encounters, plus a few that appear across many maps, such as rooms full of debris ([prefabs in depth][pre]).
Criteria that can apply to an encounter ([prefabs in depth][pre]):

- Depth limits (he has never needed them).
- Count limits: unlimited, a maximum per map, or a maximum in the whole world (once, for true uniques).
- Mutual exclusivity: encounters marked with a group exclude the rest of the group from the same map once one is chosen.
- Access limits: the room must have a given number of doors or corridor connections.
  Restricting to 1 is common, because knowing which way the player enters makes the prefab easier to design.

Weights are named, not numeric, in the data, so they read easily and can be retuned globally ([prefabs in depth][pre]).

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2017/01/cogmind_encounter_weights.png>
Source code listing the weight names and values: `-` = 0, `SR` (super rare) = 1, `R` (rare) = 5, `Low-` = 20, `Low` = 25, `Low+` = 30, `Med-` = 45, `Med` = 50, `Med+` = 55, `High-` = 70, `High` = 75, `High+` = 80, `C` (common, "pretty much guaranteed") = 100, `VC` (very common, "guaranteed, should probably be limited by number") = 200.
A comment says the plus and minus versions are for tweaking within a category and are generally avoided.

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2017/01/cogmind_encounter_weights_data_matrix_excerpt.png>
A matrix with one column per map type and one row per encounter.
Most cells are `-`; each column has a contiguous block of about ten non-empty weights, and only the top few rows have weights in several columns.
So most encounters belong to exactly one map type.

Each map type also sets the share of available rooms to fill with encounters (section 5.5) ([garrisons 2.0][gar2]).

### 6.3 Categories and their mix

Four categories ([map composition][comp]):

- Fluff: atmosphere with no mechanical effect, such as a debris-filled area.
- Rewards: unguarded rewards.
- Risk vs. reward: rewards behind an obstacle.
- Danger: hostile, with no explicit reward.

The game-wide mix in 2017 was 25% fluff, 46% rewards, 15% risk vs. reward, 14% danger ([prefabs in depth][pre]).
Garrisons 2.0 were deliberately set to 3% fluff, 40% reward, 55% risk-reward, 3% danger, because the map is already dangerous and the side rooms should be optional gambles ([garrisons 2.0][gar2]).

### 6.4 Randomization inside a prefab

From [prefabs in depth][pre] and its images.

Tag alternatives: a tag can list several names separated by `|`, one chosen at random, as in `"G-34 Mercenary|G-47 Trooper"`.
A tag can be a class instead of a name, as in `Grunt`, and the game picks the variant suited to the depth, optionally with `[CATEGORY]`, `[STRONG]` (one level above depth) and `[FALLBACK]` (use the weakest variant if none exists at or below this depth).
He prefers class tags because they survive renames and rebalancing and let one prefab serve several depths.

Items work the same way, from fully specific to fully vague: a named item, a list of categories such as `ENGINE|REACTOR`, `"Part(G-50 Soldier)"` meaning a random part that robot would carry (for themed loot), or just `ITEM`.
`[PROTOTYPE]`, `[NEVERFAULTY]` and `[RATING]+X` (out-of-depth by X levels) refine an unspecific pick.

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2017/01/cogmind_prefab_definition_syntax_randomization.png>
The two leading columns of every line.
Column 1 is an exclusion probability: `45%` means a 55% chance of ignoring this line for the whole prefab; `ea_33%` means a 67% chance of ignoring each instance of the reference character separately.
Column 2 is a group: a letter plus a number, and for each letter the loader picks one of the numbers present and keeps only lines with that number, so `a1, a1, a2` yields either the two a1 objects or the one a2 object.
Lines with a hyphen are always applied.
Exclusion probabilities are applied first, before group elimination.

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2017/01/cogmind_prefab_definition_syntax_randomization_keywords.png>
Keyword prefixes: `[10%_HACK]` applies the keyword with a 10% chance; `[global_variable_name,HACK]` applies it only if a global is true, with `!` for false and `+` to combine tests, and both forms can be combined.

Adjacency rule: when a reference character's object was chosen randomly, cardinally adjacent cells with the same character get the same object, chained as far as needed, so a row of crates holds one kind of thing.
`[UNIQUE]` turns that off ([prefabs in depth][pre]).

Whole-prefab variants: a mostly static map can have several states chosen at world generation.
The Exiles map is 51% default, 12% deserted, 12% destroyed, 25% default plus an attack during the visit, with the default forced under some world conditions ([level design][lvl]).
The deserted and destroyed variants were "basically just modified data and REXPaint maps" ([level design][lvl]).

### 6.5 Uniques across a run

Maps are generated only when the player arrives, and players visit maps in different orders.
So every unique or count-limited encounter is assigned to one valid map when the world layout is generated at the start of the run; that map may or may not end up using it, but no other map can ([seeds][seed]).
The reason given is seed consistency, so that two players on one seed see the same maps regardless of route ([seeds][seed]).

## 7. Room contents: machines, stockpiles, terminals, doors

### 7.1 Furnishing kinds

Three kinds of furnishing ([furnishing][fur]):

- Interactive machines: terminals, fabricators, scanalyzers, recycling units, repair stations, each in three levels.
- Non-interactive machines: multi-cell line-art props, grayscale, themed.
- Stockpiles: groups of 3 to 9 items split among 1 to 3 types.

Machines always occupy more than one cell (one small terminal excepted), which keeps them from being confused with robots or items and gives a sense of scale ([furnishing][fur]).
He deliberately leaves much of the floor empty for tactical clarity; machines alone "already do a good job of indicating what a given area is for" ([furnishing][fur]).
Machines break up large spaces with destructible cover, they only partially block field of view, and some explode ([non-interactive machines][nim]).

### 7.2 The composition system

Each room or hall picks one of six compositions by weighted probability set per map type ([furnishing][fur]):

1. One interactive machine, any number of non-interactive machines, and one or two stockpiles.
2. One interactive machine and any number of non-interactive machines.
3. Any number of non-interactive machines and one or two stockpiles.
4. Any number of non-interactive machines.
5. One or two stockpiles.
6. Nothing.

The weights per map type are not stated.

Placement order within an area ([furnishing][fur]):

1. The interactive machine, if any, goes first, on the edge of an inner rectangle defined by keeping a machine-to-wall padding on every side.
   It then has a chance to be pushed back against the wall, provided that blocks no door or pathway.
2. Stockpiles follow the same rule.
3. Non-interactive machines are the final filler: "the largest possible array of a randomized type of machine is placed randomly within the hall's inner rectangle", and any array slot that overlaps another object or its padding is left empty.
   Arrays are usually one machine repeated; sometimes one or more members are swapped for a different machine no larger than the rest.

Most machines may be rotated to any direction, which helps them fit ([furnishing][fur]).
Halls are processed largest to smallest, because a big empty hall most needs filling; rooms are processed in random order ([furnishing][fur]).
The number of interactive machines and stockpiles per map is capped for balance, and once a cap is reached that kind is skipped ([furnishing][fur]).
Later stages put terminals in high-traffic junctions and scatter single items, with better items preferring secluded spots ([furnishing][fur]).
The process is data-driven by "a substantial number of interrelated arrays" covering object count, relative frequency, type of location and padding; the details are not given ([furnishing][fur]).

### 7.3 Theme

Machines are tagged with a theme (mining, matter, energy, processing, production and so on), and each map type names one or more themes to draw from.
Mines use "mining" and "matter"; factories use "matter", "energy" and "production" ([non-interactive machines][nim]).
The pool is weighted, and each machine carries a separate frequency for each kind of location: room, hall and junction ([non-interactive machines][nim]).
The reason given is effort: listing every machine for every map would be a lot of work, and themes let overlapping maps share pools ([non-interactive machines][nim]).

Items use a parallel scheme: each item has an area (which map types may generate it), a depth spread (anywhere, wide across about five depths, narrow across three) and a rarity (common, uncommon, rare), and each map precalculates a weighted pool, with per-stockpile and per-map caps on a single item ([items][itm]).

### 7.4 Doors, traps, access machines

Door placement rules for ordinary rooms are not stated.
What is stated: the generator records every door and its facing ([dungeon metrics][met]); rooms seen in the images have one to three doors; hidden doors sit in back or side walls ([garrisons][gar]).

Traps come in arrays of one type, laid out in a few recognizable patterns, and he errs toward fewer rather than more ([traps][trp]).

Some map features are placed in corridors as tiny prefabs so they are seen in passing: DSF access terminals are a terminal next to one open cell, walled on three sides with reinforced wall, placed in corridors rather than rooms to raise the chance of being spotted ([maps between maps][mbm]).
Exits to some branches are "billboarded" by a recognizable surrounding layout ([procedural layouts][lay], [world of robots][wor]).

### 7.5 Images

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2014/07/cogmind_hall_placement_noninteractive.png>
A tall hall, about 9 by 20 cells of floor, with an inner rectangle drawn two cells in from the walls, a purple scanalyzer at the bottom-left of that rectangle, and a 2 by 3 grid of dark red 3x5 slots for machines; the bottom-left slot is the one dropped for being too close to the scanalyzer.

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2014/07/cogmind_hall_placement.png>
The same hall finished: five identical "Processing Tank" machines in the grid and the scanalyzer in the corner, with walking lanes two cells wide between and around them.

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2014/07/cogmind_machine_placement_electrolysis.png>
A large hall with a 3 by 4 array of identical 5x5 circular machines, two different larger machines beside it, pairs of another machine type in the lower corners, and a green terminal by a wall.
Arrays are regular and leave one-cell to two-cell aisles.

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2019/02/cogmind_factory_map_exits_marked.png>
A full 200x200 Factory map in game.
Nearly every room contains a machine array, and the arrays differ from room to room: 2x2, 2x3, 4x6 grids, single large machines, rows of bars.
Four green main access points sit near the middle of the west edge, the east edge, the south-west corner and the south-east; one yellow random main exit is in the lower middle; two orange branch exits are in the north-west quadrant ([procedural layouts][lay]).

## 8. World structure and run-to-run variety

### 8.1 Spine and branches

The world is a column of main maps from the bottom up, with branches off both sides that eventually return to the main column ([world of robots][wor]).
The quickest route is ten maps; taking branches "could easily lengthen the game to twice that or more" ([world of robots][wor]).
There is no backtracking: leaving a map is permanent ([world of robots][wor]).
"Combine this with the lack of backtracking and you'll find that it's impossible to visit every location in a single play" ([world of robots][wor]).
He says the purpose is not replayability, which procedural content already gives, but forcing an experienced player "to make decisions that give up one potential benefit for another" ([world of robots][wor]).

He expanded the world "horizontally rather than vertically", for room, and because each depth is expected to bring stronger content, and a short ladder lets each rung be distinct instead of a rehash with bigger numbers ([world of robots][wor]).
Branches extend at most four maps out to either side ([world map][wmap]).

The main complex is intentionally the tamer, more predictable part: "a sort of hub-like area where you can relax your mind a bit", consistent across depths so that players can plan ([cargo convoys][cc]).
Branches hold the unusual encounters, the best parts, allies, and nearly all the story ([world of robots][wor], [map composition][comp]).
Main maps are the largest but "a minority of the total area of the world" ([cargo convoys][cc]).

### 8.2 The layout changes every run

The whole world layout is generated at the start of a run from the world seed, and "that layout is generated anew procedurally with each run" ([world map][wmap], [seeds][seed]).
How branches attach "follows a set of rules that you can gradually learn", such as the first three main areas always being Materials and Mines entrances always being in the first two ([world map][wmap]).
Many branch links "can appear at more than one depth" ([cargo convoys][cc]).
A special map may exist at one of two depths but only one per run; the Exiles appear at -10 or -9, and a log line on entering a map (a "level feeling") tells an observant player they are at the right depth so they do not search in vain ([level design][lvl]).
Each map's seed is derived from the world seed and stored in the world layout, so a map is the same however it is reached ([seeds][seed]).

By design the player is somewhat more likely to find branch exits before the exit upward, so detours happen by necessity as well as by choice ([world of robots][wor]).
Unidentified exits all look alike until hacked, so taking one is a gamble ([world of robots][wor]).

### 8.3 Maps between maps

A third category, intermaps, are small optional maps reached by special entrances: Waste (via chute traps), Garrisons (via a hackable access machine) and DSFs (via a terminal that locks permanently once any hostile spots the player) ([maps between maps][mbm]).
Their shared traits: reachable from several depths, several access points per parent map, unique entry method, small, strongly themed, optional, strategically meaningful, and the only way to loop to another variant of the same depth ([maps between maps][mbm]).
He compares them to vaults in other roguelikes, built as separate maps because of the no-backtracking rule ([maps between maps][mbm]).

### 8.4 Layers of rarity

- Encounter weights run from 1 (super rare) to 200 (very common), a 200 to 1 spread ([prefabs in depth][pre]).
- Events sit on top of encounters and are rarer still.
  For Garrisons: a 60% chance of exactly one event across a run that visits a Garrison at every depth, 10% for two, and 30% of seeds have none ([garrisons 2.0][gar2]).
  His reason: once players know the individual encounters, an occasional map-wide change keeps the map from being a flat "move from point A to B".
- Main-complex surprise events were kept "mostly rare encounters that only people who play a lot will ever see" ([cargo convoys][cc]).
- Roaming content adds variety without new rooms: a cargo convoy crosses the map along a route, announced in advance, one at a time, entering far from the player ([cargo convoys][cc]).
- Wandering actors mean a cleared passage "might not be so clear on the next visit", which he prefers to autoexplore as the answer to empty space ([backtracking][bt]).

### 8.5 Static versus procedural

He classifies Cogmind's map types as 9 procedural, 13 mixed and 8 static, and by map count the procedural and mixed are each at least twice the static ([static or procedural][sp]).
He treats it as a spectrum: static maps with procedural bits, and procedural maps with pieces that are fully static or merely "guaranteed to be on that map" somewhere ([static or procedural][sp]).
Advantages he gives for static content: anchoring lore, letting players plan around known checkpoints, change of pace, and shared landmarks for the community ([static or procedural][sp]).
"Larger areas are more suited to procgen" ([static or procedural][sp]).

### 8.6 Images

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2015/04/cogmind_world_composition.png>
A diagram: a central "Main Complex" column from the player's start at the bottom to "Surface" at the top, flanked by two "Branches" columns, with small arrows crossing between the columns in both directions at about six heights.

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2021/09/cogmind_pixelwise_world_map_sample_no_redacted_spoilers.png>
One generated world at one pixel per cell.
A central column of about ten main maps that grow from small at the bottom to large squares in the middle and top.
Tan cave maps fan out to the left in chains of two or three.
Small red cross-shaped Garrisons are attached beside almost every main map.
Green, yellow, orange and blue room-and-corridor branch maps of various sizes and aspect ratios hang to the right, including one long thin horizontal map.
Lines join each branch to the main map it leaves from and returns to; many return one depth higher.

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2021/12/cogmind_average_accessible_map_cell_count_samples.png>
A stacked bar chart of open cells and solid terrain per map type; values are in section 9.

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2019/02/cogmind_maps_static_vs_procedural_distribution.png>
A bar chart: Procedural 9, Mixed 13, Static 8.

## 9. Map sizes, pacing and exits

### 9.1 Sizes

- The 7DRL prototype's maps were 100x100; the full game's range from 100x100 to 200x200 ([tunneling][tun], [prototype to full game][pfg]).
- The largest main maps are 200x200 ([procedural layouts][lay]).
- Cave maps are smaller than main maps; mines are small squares ([cellular automata][ca], [caves][cav]).
- Average open cells: main maps about 16,000, branch maps about 4,000, intermaps about 1,000 ([maps between maps][mbm]).
- Read off the bar chart in [maps between maps][mbm] (approximate, my reading): Factory about 40,000 total with 20,000 open; Research 40,000 with 18,000 open; Access 45,000 with 22,500 open; Materials about 5,500 total with 3,600 open; Mines and Storage about 5,500 total with 1,500 to 2,000 open; the three cave types about 19,000 total with 3,800 open; Armory 15,000 with 6,700 open; Quarantine and Testing 20,000 with 3,800 to 4,500 open; Waste and Garrison 10,000 with about 1,100 open; DSF 2,500 with 700 open.
  The Materials figure does not match the roughly 100x100 coordinate space of the Materials feature table in section 5.2; the reason is not stated.
- So main maps are about half open, and caves and intermaps are 10 to 20 percent open.
- The view shows up to 50x50 cells of the map at once, and all mechanics assume that ([POLYBOT-7 part 3][p7]).
- Prefab sizes: average 7x7, largest 100x90 ([prefabs in depth][pre]).

### 9.2 Why large

Reasons given in [tunneling][tun]: ranged combat needs space; the larger world needs area transitions to be less frequent; bigger battles; larger maps make intel gathering (terminals, sensors) worth doing without making it a guaranteed win; and machines now fill space that was empty, so less is occupiable than it looks.
More open space also "forces you to think a bit more before attracting attention", where the corridor-heavy 7DRL made hiding seem easier than it was ([tunneling][tun]).
Empty rooms and corridors have a purpose: rest, room to maneuver when things go wrong, and space for stealth ([procedural map generation][pmg]).
"Compartmentalizing encounters into rooms enables dungeons to provide a tighter experience, fitting more content within a smaller overall area" ([procedural map generation][pmg]).

### 9.3 Not meant to be cleared

"It's not usually in your best interest to explore the entirety of every map in Cogmind--they're big and you're likely to lose more than you gain from doing so" ([backtracking][bt]).
The pressure comes from clocks: unrepairable core damage restored only by ascending, and alert that rises with activity ([world of robots][wor], [garrisons 2.0][gar2]).
The Garrison clock is concrete: ambient alert starts accumulating at turn 1200, ramps quickly, and then the rate (not the alert) plateaus, which makes it a soft limit and not a wall ([garrisons 2.0][gar2]).
He prefers flexible clocks to hard cutoffs ([garrisons 2.0][gar2]).

### 9.4 Exits control difficulty

"These exits cannot be made too easy to find, as their frequency and locations essentially control the overall difficulty of the game" ([garrison access][ga]).
Because reaching the next map is the reward (it heals, upgrades and sheds pursuit), "the absolute most important factor for balance is the placement of entrances and exits" ([procedural layouts][lay]).
Main exits are several per map, far apart, with "a guaranteed amount of space between them" ([tunneling][tun]).
The two Factory samples have four and three pre-placed main access points plus one and two randomly placed main exits; the first has two branch exits and the second "connects to more branch maps" (five orange markers in its image, by my count) ([procedural layouts][lay]).
Main maps have spawn protection near the entry; branches do not ([backtracking][bt]).

Loops and dead ends: he wants "enough looping paths providing alternative routes to reach different areas, meaning fewer dead ends", more so as maps get larger ([backtracking][bt]).
A long path to a dead end "had better at least contain treasure or access to a new area" ([procedural map generation][pmg]).

### 9.5 POLYBOT-7: the same generator, small

POLYBOT-7 (2018 7DRL) uses the Cogmind tunneler "with different parameters" ([POLYBOT-7 part 3][p7]).
It has five floors and map size increases with depth: 80x80 on the second floor, 100x100 on the third, 125x125 on the fourth ([POLYBOT-7 part 3][p7]).
The first and fifth floor sizes are not stated.
Each floor has "multiple different layouts" (styles), built for replay variety, and that took a full day of the seven ([POLYBOT-7 part 3][p7]).
It has "a lot more hidden doors and corridors than Cogmind" and "very few large rooms" ([POLYBOT-7 part 3][p7]).
Slot module rooms, the source of permanent upgrades, are prefabs placed as mapgen parameters "in certain locations relative to map entrances/exits", usually three per map, sometimes four ([POLYBOT-7 part 3][p7]).

The design lesson he draws: smaller maps made it hard to keep exits far apart, so he moved the reward from the exit to things found on the floor.
With upgrades local to a floor, "I no longer have to spread out the exits", because players stay to explore for upgrades as long as they dare ([POLYBOT-7 part 3][p7]).
Ranges were scaled by 0.6 to match a 30-cell view instead of 50 ([POLYBOT-7 part 3][p7]).

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2018/03/polybot7_procedural_map_structure_annotated.png>
A 100x100 map, annotated.
Four red entrance/exit dots cluster in the center of the map, about 10 to 20 cells from each other.
Four green "slot module rooms" sit one in each corner region, each in a small black-margined box at the end of a corridor.
Four orange "open halls" are spread around the middle ring.
About sixty gray rooms, mostly small and squarish.
A dozen cyan hidden corridors and doors join adjacent rooms.
Corridors are 1 to 4 wide and loop everywhere.
So the layout is the inverse of Cogmind's: exits in the middle, rewards at the periphery.

Image: <https://www.gridsagegames.com/blog/gsg-content/uploads/2018/03/polybot7_mapgen_tests_125_cubbies.gif>
A 104-frame preview of repeated generations of one 125x125 style.
Frames taken before tunneling show only four small white cubby prefabs, one near each corner, on an otherwise solid map, at slightly different positions each time, which shows seeding with a random offset.
A finished frame shows a dense room cluster with the most secluded rooms shaded red at the edges.

## 10. What this suggests for foundry (inference, not sourced fact)

Everything in this section is my inference from the sources above, measured against the current state of `examples/foundry` and `crates/atglyph-mapgen`.
None of it is something Josh Ge said about this project.

Where foundry is now, from a brief read of the repo: a deck is 70x40 (2,800 cells), built by a rooms pass with L-corridors, with required pieces stamped first (armory, store, guard post, reactor or core) and then one centered prefab per remaining room from six room kinds in two sizes (`examples/foundry/src/decks.rs`, `crates/atglyph-mapgen/src/dungeon.rs`, `crates/atglyph-mapgen/src/prefab.rs`).
Prefabs are RON with rows, a legend, tagged item and monster draws by band, and `Mark` cells (`examples/foundry/assets/prefabs/*.ron`).
The engine already has rotation, flipping, `StampOneOf` weighted choice and `Placement::AnyRoom`.

### 10.1 Unpredictable

1. Several layout plans per deck, one drawn per generation.
   This is the technique he aims squarely at the owner's objection.
   A single "main corridor then subdivide" plan is predictable because it is one plan.
   Materials has three (cross, spiral, corridors) that differ in where exits sit, where barriers go and which direction growth starts, and the player cannot tell which is in play.
   For foundry that means a small set of plans per deck theme, each a handful of numbers: exit regions with jitter, start directions, optional barriers with a probability, reserved blocks.
2. Grow from the points of interest outward and let the pieces collide.
   Routes become circuitous because independent diggers meet where they happen to meet.
   This is the opposite of carving a spine first.
3. Diggers that change width and behavior as they go.
   This is what gives one map both a 5-wide trunk and 1-wide service crawlways, and it is why his maps do not read as a grid.
   The current L-corridor pass has one width and one shape.
4. Random barriers.
   A wall the generator may not cross, present with some probability, changes the topology of a whole region for the cost of one rectangle.
5. Reject and retry against metrics.
   A wilder generator is affordable only if bad outputs are thrown away.
   The engine's seed-range property tests are already the same idea in test form; the suggestion is to make the checks part of the build chain with a bounded retry count.
6. Hidden room-to-room links.
   In a factory these read naturally as maintenance ducts or service hatches, and they are his only direct room-to-room connection.

The gap to be aware of: the posts do not give the tunneler's step rules.
A foundry design will have to write those rules itself, with the DungeonMaker manual as the stated ancestor.

### 10.2 Factory feel

1. Arrays of one machine, filled to the largest grid that fits, with slots dropped where something else stands.
   Looking at his Factory maps, this is most of what makes them read as a factory: rooms hold a 2x3 of presses or a 4x6 of tanks, not one centered set piece.
   Foundry's one-centered-prefab-per-room is the main thing those images argue against.
2. Halls as a distinct place type, furnished largest first.
   Big open floors with big machines as cover are where a factory looks like one.
3. Machine themes per deck, with separate weights for room, hall and junction.
   A deck could draw from two or three themes (stamping, coolant, power, assembly), which gives decks an identity cheaply and fits the registry rule.
4. Terminals at rated junctions.
   A console at a corridor crossing is a factory image and it gives junctions a job.
5. The six compositions including "nothing".
   Foundry's design says a blank room is to be avoided, and he agrees that an area should say what it is for, but he also keeps empty floor for tactics and has an explicit empty composition.
   The reconciliation that fits both is that a room with only a machine array and no encounter is "furnished but quiet".
6. Wide main corridors if ranged combat matters; his 3-wide standard is derived from firing lines.

### 10.3 Larger and worth exploring

1. Size relative to view matters more than absolute size.
   His maps are 2 to 4 view-widths across (50-cell view, 100 to 200 map), and POLYBOT-7 scaled to 80 to 125 with a 30-cell view.
   A 70x40 deck is likely close to one screen.
   "Larger but not huge" in his terms would be about 2 to 3 views per side, in the POLYBOT-7 range and not the Factory range.
2. Size could grow by deck, as POLYBOT-7 does.
3. Seclusion-weighted rewards.
   Computing shortest paths between entry and exit and rating rooms by distance from them is cheap with the engine's `DijkstraMap`, and it is what makes leaving the direct route pay.
4. Reward on the floor, not at the exit, if maps stay smallish.
   This is the POLYBOT-7 lesson and it fits foundry's reactor charges and build-defining finds: put the things worth having at the periphery, and then the exit need not be far away.
5. Loops over dead ends, checked as a metric, and a dead end must hold something.
6. A clock that makes full clearing a choice.
   He assumes players will not see the whole map, which is what lets a map hold more than a run consumes.

### 10.4 More content than one run shows

1. An encounter table with named weights, per-deck pools, and count limits per map and per run.
   The 1 to 200 weight spread, with most encounters belonging to one map type, is the mechanism behind "you will not see everything".
   Foundry currently has six room kinds; his game-wide number is 148 encounter types over about thirty map types, and Garrisons alone had 44 quadrants and still felt repetitive at ten visits per run.
2. Fill only a share of rooms with encounters (his range is 10% to 100% by map type) and let the composition system furnish the rest.
   That separates "what makes this room a factory room" from "what is special here", and it is what lets the special things be rare.
3. Multiple prefab alternatives per encounter, in different footprints, so the encounter fits more rooms and looks different each time (he has up to 12).
   Foundry already does this at two sizes.
4. Randomization inside prefabs: per-line exclusion chances, pick-one groups, class tags resolved by depth, the same-adjacent rule.
   Foundry's tagged draws by band already cover the class-tag case; exclusion chances and groups are the missing pieces.
5. Assign uniques to a deck at run start.
   With ten decks and a per-run draw, some set pieces simply do not exist in a given run.
6. Whole-prefab state variants chosen at run start (intact, abandoned, wrecked, under attack), which multiply content at the cost of an alternate file.
7. Rare deck-wide events as a second layer above encounters.
8. Optional side maps.
   His strongest "cannot see it all" lever is the branch structure with no backtracking.
   Foundry is a ten-deck descent and return, so the nearest equivalents are optional sub-areas per deck that appear only in some runs, or intermap-style small themed maps behind a special entrance.
   That is a larger design decision than the others and is noted here only as the direction the source points.

### 10.5 Engine-side implications

- Embedding needs door facing: his enclosed-room embed rotates the prefab to face the room's door and shrinks the room to the prefab.
  The engine's `Placement::AnyRoom` would need the room's doors as an input, and a refill-and-rewall step.
- Seeding needs a "reserved, off limits" region and a connector stub that a digger starts from.
- The layout phase should publish junctions, halls and doors as outputs alongside `Room`, since furnishing, guards and terminals all key off them.
- Keep the three-phase split (layout, content, player-affected) so a seed gives the same deck however the player arrives; the engine's `RunSeed::derive` per map is already his per-map seed.

## 11. Sources

All fetched 2026-09-30 from www.gridsagegames.com with curl and read as text; dates are the posts' own publish dates.

Read in full:

| Key | Post | Date |
| --- | --- | --- |
| [pmg] | Procedural Map Generation - <https://www.gridsagegames.com/blog/2014/06/procedural-map-generation/> | 2014-06-13 |
| [tun] | Mapgen: Tunneling Algorithm - <https://www.gridsagegames.com/blog/2014/06/mapgen-tunneling-algorithm/> | 2014-06-20 |
| [ca] | Mapgen: Cellular Automata - <https://www.gridsagegames.com/blog/2014/06/mapgen-cellular-automata/> | 2014-06-27 |
| [met] | Dungeon Metrics - <https://www.gridsagegames.com/blog/2014/07/dungeon-metrics/> | 2014-07-04 |
| [dp] | Dungeon Prefabs - <https://www.gridsagegames.com/blog/2014/07/dungeon-prefabs/> | 2014-07-12 |
| [fur] | Furnishing a/the Dungeon - <https://www.gridsagegames.com/blog/2014/07/furnishing-athe-dungeon/> | 2014-07-19 |
| [nim] | Non-Interactive Machines - <https://www.gridsagegames.com/blog/2014/07/non-interactive-machines/> | 2014-07-28 |
| [wor] | A World of Robots - <https://www.gridsagegames.com/blog/2015/04/world-robots/> | 2015-04-21 |
| [comp] | Map Composition - <https://www.gridsagegames.com/blog/2015/05/map-composition/> | 2015-05-12 |
| [ga] | Garrison Access - <https://www.gridsagegames.com/blog/2015/11/garrison-access/> | 2015-11-04 |
| [gar] | Garrisons - <https://www.gridsagegames.com/blog/2015/11/garrisons/> | 2015-11-10 |
| [wmap] | Designing Cogmind's World Map - <https://www.gridsagegames.com/blog/2016/02/designing-cogminds-world-map/> | 2016-02-02 |
| [cav] | Generating and Populating Caves - <https://www.gridsagegames.com/blog/2016/03/generating-populating-caves/> | 2016-03-15 |
| [pre] | Map Prefabs, in Depth - <https://www.gridsagegames.com/blog/2017/01/map-prefabs-in-depth/> | 2017-01-11 |
| [p7] | 2018 7DRL Postmortem, Part 3: Spending Too Long on Map Generation and Content - <https://www.gridsagegames.com/blog/2018/03/7drl-postmortem-part-3-spending-long-map-generation-content/> | 2018-03-26 |
| [div] | Debugging Mapgen Seed Divergence - <https://www.gridsagegames.com/blog/2018/11/debugging-mapgen-seed-divergence/> | 2018-11-14 |
| [lvl] | Level Design and Shaping a Cogmind Experience - <https://www.gridsagegames.com/blog/2019/02/level-design-shaping-cogmind-experience/> | 2019-02-19 |
| [lay] | Roguelike Level Design Addendum: Procedural Layouts - <https://www.gridsagegames.com/blog/2019/03/roguelike-level-design-addendum-procedural-layouts/> | 2019-03-06 |
| [sp] | Roguelike Level Design Addendum: Static or Procedural? - <https://www.gridsagegames.com/blog/2019/03/roguelike-level-design-addendum-static-procedural/> | 2019-03-28 |
| [cc] | Spicing Up Primary Maps 1: Cargo Convoys - <https://www.gridsagegames.com/blog/2021/09/spicing-up-primary-maps-1-cargo-convoys/> | 2021-09-14 |
| [mbm] | Maps Between Maps, and DSFs - <https://www.gridsagegames.com/blog/2021/12/maps-between-maps-and-dsfs/> | 2021-12-10 |
| [gar2] | Garrisons 2.0 - <https://www.gridsagegames.com/blog/2022/07/garrisons-2-0/> | 2022-07-18 |
| - | Machines - <https://www.gridsagegames.com/blog/2013/10/machines/> | 2013-10-14 |

Read in part (searched for passages on maps, generation, prefabs, exits, sizes and variety, and those passages read):

| Key | Post | Date |
| --- | --- | --- |
| [itm] | Items: Types & Distribution - <https://www.gridsagegames.com/blog/2014/02/item-types-distribution/> | 2014-02-28 |
| [pfg] | From Prototype to Full Game - <https://www.gridsagegames.com/blog/2014/10/prototype-full-game/> | 2014-10-01 |
| - | The Living Dungeon - <https://www.gridsagegames.com/blog/2015/04/living-dungeon/> | 2015-04-30 |
| [trp] | Traps - <https://www.gridsagegames.com/blog/2015/08/traps/> | 2015-08-18 |
| - | Weaving Narratives into Procedural Worlds, Part 3: Methods - <https://www.gridsagegames.com/blog/2016/11/weaving-narratives-procedural-worlds-part-3-methods/> | 2016-11-10 |
| [seed] | Working with Seeds - <https://www.gridsagegames.com/blog/2017/05/working-seeds/> | 2017-05-02 |
| - | Beyond the Design Doc: Unplanned Cogmind Features - <https://www.gridsagegames.com/blog/2017/07/beyond-design-doc-unplanned-cogmind-features/> | 2017-07-25 |
| - | POLYBOT-7, a 2018 7DRL - <https://www.gridsagegames.com/blog/2018/03/polybot-7-2018-7drl/> | 2018-03-12 |
| - | How to Make a Roguelike - <https://www.gridsagegames.com/blog/2018/10/how-to-make-a-roguelike/> | 2018-10-24 |
| - | Special Mode Design: Abominations - <https://www.gridsagegames.com/blog/2019/10/special-mode-design-abominations/> | 2019-10-26 |
| - | My Cogmind Dev Tools - <https://www.gridsagegames.com/blog/2020/05/cogmind-dev-tools/> | 2020-05-11 |
| - | Exploring the Concept of a Terminal Roguelike "Overmap" - <https://www.gridsagegames.com/blog/2020/11/exploring-concept-terminal-roguelike-overmap/> | 2020-11-09 |
| [des] | "Desire Paths" for a Robot World - <https://www.gridsagegames.com/blog/2021/07/desire-paths-for-a-robot-world/> | 2021-07-22 |
| [den] | Spicing Up Primary Maps 2: Area Denial - <https://www.gridsagegames.com/blog/2021/09/spicing-up-primary-maps-2-area-denial/> | 2021-09-28 |
| - | Kyzrati's Game Design Philosophy - <https://www.gridsagegames.com/blog/2022/05/kyzratis-game-design-philosophy/> | 2022-05-08 |
| [bt] | On Backtracking in Roguelikes - <https://www.gridsagegames.com/blog/2022/11/on-backtracking-in-roguelikes/> | 2022-11-29 |

How the posts were found: the blog has no mapgen or prefab tag (its tag list is balance, cogmind, gamedev, gui, multi-tile, pathfinding, progress, roadmap, scaling, ui), so all 46 index pages (227 posts) were crawled and the list read by title.
The blog's own category for the topic is <https://www.gridsagegames.com/blog/category/dev-series-procedural-maps/>, which holds five of the posts above.

Not found or not fetched:

- No post gives the tunneler's step-by-step rules or parameter values; see section 2.4.
- No post is dedicated to one main map type's generator parameters (Factory, Materials, Research, Storage, Access); what exists is the Materials feature table and layout plans, and the Factory and Storage samples.
- No post gives door placement rules or composition weights per map type.
- The linked design documents in [lvl] (the Exiles rough notes and map design doc as text files) and the full-spoiler world map image were not read.
- Off-blog first-party material was searched for but not read: a web search for his r/roguelikedev FAQ Friday answers on map generation and for a Roguelike Celebration talk on the topic returned only the blog posts above and reposts of them.
  His Roguelike Celebration 2018 talk is "How to Make a Roguelike", and its blog write-up has nothing on map generation beyond a mention that libtcod provides it.
- The DungeonMaker manual he credits (<http://dungeonmaker.sourceforge.net/DM2_Manual/manual1.html>) and Andy Stobirski's cave generator write-up are third-party and were not read.
- A request to the blog with a truncated browser user-agent string was rejected by the server (HTTP 406); all pages listed were then fetched with curl's default user agent without any problem.

[pmg]: https://www.gridsagegames.com/blog/2014/06/procedural-map-generation/
[tun]: https://www.gridsagegames.com/blog/2014/06/mapgen-tunneling-algorithm/
[ca]: https://www.gridsagegames.com/blog/2014/06/mapgen-cellular-automata/
[met]: https://www.gridsagegames.com/blog/2014/07/dungeon-metrics/
[dp]: https://www.gridsagegames.com/blog/2014/07/dungeon-prefabs/
[fur]: https://www.gridsagegames.com/blog/2014/07/furnishing-athe-dungeon/
[nim]: https://www.gridsagegames.com/blog/2014/07/non-interactive-machines/
[wor]: https://www.gridsagegames.com/blog/2015/04/world-robots/
[comp]: https://www.gridsagegames.com/blog/2015/05/map-composition/
[ga]: https://www.gridsagegames.com/blog/2015/11/garrison-access/
[gar]: https://www.gridsagegames.com/blog/2015/11/garrisons/
[wmap]: https://www.gridsagegames.com/blog/2016/02/designing-cogminds-world-map/
[cav]: https://www.gridsagegames.com/blog/2016/03/generating-populating-caves/
[pre]: https://www.gridsagegames.com/blog/2017/01/map-prefabs-in-depth/
[p7]: https://www.gridsagegames.com/blog/2018/03/7drl-postmortem-part-3-spending-long-map-generation-content/
[div]: https://www.gridsagegames.com/blog/2018/11/debugging-mapgen-seed-divergence/
[lvl]: https://www.gridsagegames.com/blog/2019/02/level-design-shaping-cogmind-experience/
[lay]: https://www.gridsagegames.com/blog/2019/03/roguelike-level-design-addendum-procedural-layouts/
[sp]: https://www.gridsagegames.com/blog/2019/03/roguelike-level-design-addendum-static-procedural/
[cc]: https://www.gridsagegames.com/blog/2021/09/spicing-up-primary-maps-1-cargo-convoys/
[mbm]: https://www.gridsagegames.com/blog/2021/12/maps-between-maps-and-dsfs/
[gar2]: https://www.gridsagegames.com/blog/2022/07/garrisons-2-0/
[itm]: https://www.gridsagegames.com/blog/2014/02/item-types-distribution/
[pfg]: https://www.gridsagegames.com/blog/2014/10/prototype-full-game/
[trp]: https://www.gridsagegames.com/blog/2015/08/traps/
[seed]: https://www.gridsagegames.com/blog/2017/05/working-seeds/
[des]: https://www.gridsagegames.com/blog/2021/07/desire-paths-for-a-robot-world/
[den]: https://www.gridsagegames.com/blog/2021/09/spicing-up-primary-maps-2-area-denial/
[bt]: https://www.gridsagegames.com/blog/2022/11/on-backtracking-in-roguelikes/
