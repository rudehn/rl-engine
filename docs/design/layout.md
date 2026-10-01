# Layout

Status: designed 2026-09-30, not built.
This is the spec the first implementation is written against; the documentation pass that finishes the branch rewrites it as what was built.
The research behind it is `docs/research/gridsage-mapgen.md`, read for its concepts and not for its algorithms.

## 0. Summary

A place built by `Rooms` is rectangles scattered in rock and joined by whichever corridor reached the last one.
It reads as a dungeon, it is the same everywhere, and nothing about it can be asked a question: no pass knows which room is a dead end, which door is the only way through, or which corner is far from the way out.

This subsystem builds a place by accretion instead.
A first piece is laid down, and every later piece is attached to a wall of what already stands, with an opening between, so the place is connected by construction and its rooms share walls the way a built thing's do.
Which shapes are drawn depends on where on the map the piece is going, so a place has regions that differ, and after growth the longest detours are cut through so there is more than one way round.

Then the place is surveyed, and the survey is published: every area, every door, and for each what a later pass needs to decide what belongs there.
A place that fails what its recipe requires is thrown away and built again from the next attempt of the same seed.

It is the first of four pieces of work, and the only one this document specifies.
The other three key on what this one publishes: a furnisher that fills an area to its size, a director that decides what a run holds and where, and tells that show a thing before it is reached.

## 1. What it is for

Six things make a generated place worth exploring, and each section below serves one or more of them.

1. The player learns rules, never layouts: a grammar that can be reasoned about, with an instance that cannot be guessed.
2. A place is decisions and not an area to cover, so it may hold more than a visit sees.
3. Geography has a gradient: what is off the direct route is worth more, there is more than one way round, and a dead end pays.
4. Places have identity and contrast, in the scale of a room and in the scale of a region.
5. Rarity comes in layers over a quiet baseline.
6. Anticipation beats surprise.

This piece of work delivers the structure for the first four: unpredictable growth, loops, contrast between regions, and the survey that lets later work put the right thing in the right place.
The fifth and sixth are the director's and the tells'.

A layout plan that carves the circulation first and subdivides what is left was considered and refused: it is one plan, and a player has learned it by the second place.

## 2. Growth

### 2.1 Shapes

A `Shape` draws one `Piece` from a stream: the floor cells of its body, relative to its own corner, and its sites.
A site is a wall cell just outside the body, with the direction that leads out of the body through it, where the piece may be joined to another.
A shape also says how it is joined: by a door, by an open arch one cell wide, or by a mouth as wide as the piece allows.

`Shape` is a trait, so a game draws a piece the engine never thought of.
The engine ships three:

- a rectangle, between a smallest and a largest side;
- a cross, two rectangles overlapped on their centers;
- a passage, a width, a length and a chance of one bend.

A hall is not a fourth: it is a rectangle drawn from a larger range and joined by a mouth, which a game's file says in one row.
A shape also says whether what it draws is a room, somewhere a piece may be stamped; a passage is not.

A prefab as a shape, attached by a door drawn in the piece, is the director's and is not built here.
The trait is the seam it will use.

A shape has a name, interned as the game's other names are.
An area remembers the name of the shape that drew it, and nothing in the engine reads a meaning into that name: "hall" is a string in a game's file.
There is no enum of area kinds, for the reason there is none of content.

### 2.2 The loop

`Accrete` is a pass in `Phase::Structures`.
It runs over a map filled with solid tile and keeps a one-cell border solid.

1. Draw a piece from the district at a random point of the map and lay it there, as the first area.
2. Draw a site from the frontier: the sites of every area laid so far that still face solid tile.
3. Draw a shape from the district the site is in, and a piece from the shape.
4. For each of the new piece's sites that faces the opposite way, in a drawn order, place the piece so the two sites are the same cell, which is the wall between them.
   It fits when every cell of its body, and the ring one cell round it, is solid and inside the border, the joining cell excepted.
5. When it fits, carve the body, open the joining cell as the shape says, and record the area and the opening.
   When no site fits, the attempt is spent.
6. Stop when the share of the map that is open reaches the recipe's target, or when a run of consecutive attempts as long as the recipe allows has all been spent.

Every area is joined to one that was already joined, so every open cell can be walked to from every other, before any check is made.
The ring in step 4 is what keeps one wall between two areas and never none.

Every number above is a share or a count per area of map, never a count of cells of map: the target is a share, and the allowed run of failures is the recipe's.
The same recipe therefore builds a small place and a large one, and the tests in section 7 build both.

### 2.3 Districts

Before growth, district seeds are scattered: as many as the recipe's density per ten thousand cells gives, at least one, each drawn a district from the recipe's weighted list.
A cell is in the district of its nearest seed, by squared distance, ties to the earlier seed.

A district is a name and a weighted list of shapes.
It biases a draw and builds nothing: no wall follows a district's edge, and a piece drawn in one district may reach into the next.
So a region of wide halls gives way to a knot of narrow passages without a seam, and a recipe with one district is the generator with none.

Districts are kept, at a size where each holds perhaps ten areas, because the cost is a nearest-seed lookup and the alternative is a place whose mix is the same in every corner, which shows most as a place grows.
An area records its district, so the furnisher can theme a region and the director can keep an encounter to the kind of region it belongs in.

### 2.4 Loops

Accretion alone builds a tree: one way to anywhere.
`OpenLoops` is its own pass, straight after `Accrete`, with its own stream.
After growth, every wall cell with open floor on two opposite sides, belonging to two different areas, is a candidate.
Its detour is the walking distance between those two floor cells as the place stands.
The candidate with the longest detour is opened, as a door, while that detour is at least the recipe's threshold; distances are measured again after each, since one opening shortens others.
The recipe also caps how many are opened.

Longest first is what makes a loop worth having: an opening that saves three steps is a second door, and one that saves sixty is another way round.

## 3. What growth publishes

- `Area`, one per piece: an index, the shape's name, the district's name, its cells, their bounds, and the largest rectangle that fits inside it.
- `Opening`, one per join and per loop: its cell, its facing, the two areas it joins, and how it was joined.
- `Room`, the existing output, one per area whose shape draws rooms, holding the area's largest inner rectangle.

`Room` is what `StampPrefab`, `StampOneOf` and `StampEachRoom` already read, so every existing stamp works on an accreted place unchanged, and a passage, which emits none, is never furnished or chosen for a vault.
`Doors` is not used with `Accrete`, which opens its own.

## 4. The survey

`Survey` is a pass in `Phase::Exits`, run after the start and the exit are known.
It reads the areas, the openings and the terrain, and emits one `Surveyed`.

For each area:

- how many openings it has;
- whether it is a dead end, which is one opening;
- whether it is a chokepoint, which is an area whose removal leaves two others with no way between them;
- its seclusion: the walking distance from its nearest opening to the nearest cell of the shortest route from the start to the exit.

For the place:

- the junctions: areas with three or more openings;
- the number of independent loops in the graph of areas and openings, which is openings less areas plus one;
- the share of the map that is open;
- the walking distance from the start to the exit.

Seclusion is the one a reward keys on: what is far from the direct route is what a player chose to go and find.
A place with no exit has no route, and its seclusion is the distance from the start.

The survey decides nothing.
It is in the engine because every game that places anything by where it is would otherwise measure the same things again, differently.

## 5. Requirements and retry

`Require` is a pass in `Phase::Finish` holding the bounds a recipe states:

- the open share, at least and at most;
- the fewest areas drawn by a named shape, for as many names as the recipe lists;
- the fewest loops;
- the largest share of areas that may be dead ends;
- the shortest walking distance from the start to the exit.

It fails with the first bound the survey does not meet, naming the bound and both numbers.

A loose generator is only safe if what it gets wrong is thrown away, and the module docs of `rl-mapgen` already say a caller retries a failed chain with a different index.
Nothing does: `Chain::run` draws every pass's stream at index nought, and every game propagates the error.
So `Chain::run_retrying(make, seed, attempts)` is added: it builds a fresh context with `make`, runs the chain with every pass's stream derived at the attempt's index, and returns the first that succeeds, or the last error once the attempts are spent.
The same seed therefore builds the same place however many attempts it took, and `Chain::run` is attempt nought.

## 6. The recipe file

A recipe is data, and its file is the engine's, as a prefab's is.
`rl-rules` owns the file and what is wrong with it, and names no mapgen type; `rl-bevy` turns a loaded recipe into the passes, the way `Prefabs::piece` turns a loaded prefab into a `rl_mapgen::Prefab`.
So a recipe loads, and every mistake in it is reported by name, with no map and no `App`.

A layout file holds the shapes, the districts and the recipes:

- a shape: a name, and one of the three the engine ships with its ranges and how it is joined;
- a district: a name and `(shape, weight)` rows;
- a recipe: a name, a width and a height, the target open share, the run of failed attempts allowed, the district density, `(district, weight)` rows, the loop threshold and cap, and its requirements.

Checked at load: every name resolves, every weight is more than nought, every range has its smallest no larger than its largest, every share is between nought and a hundred, the requirements' shape names are shapes some district of the recipe can draw, and the smallest shape fits the recipe's size.
The file carries the top-of-file comment listing every field and every option.

A game with a `Shape` of its own builds its `Accrete` in code; naming one from a file waits for a game that has one.

## 7. Foundry

`assets/layout.ron` holds foundry's shapes, three districts and its recipes.
The districts are halls, cells and service passages, and a deck's recipe weights two or three of them.
Each deck names its recipe, so a deck's size is data and the `70, 40` in `decks.rs` goes.

A deck starts at 84 by 48, about 1.4 times the area it has now.
Accretion wastes no rock between rooms, so that holds noticeably more than the scatter does, and it stays small while there is little to put in it.
The map view already follows the player and clamps to the map, so a deck larger than the screen needs nothing new.

The deck's chain swaps `Rooms` and `Doors` for `Accrete` and `OpenLoops`, and adds `Survey` after `FarthestExit` and `Require` last.
The armory, the store, the guard post, the reactor, the core and the furnishing stay as they are, stamped into the rooms accretion emits, until the furnisher and the director replace them.
`Foundry::build` runs the chain retrying.

## 8. Testing

In `rl-mapgen`, with no `App`, over a range of seeds and at two sizes, 84 by 48 and 160 by 90:

- every open cell of an accreted place can be walked to from every other;
- no opening has rock on either side of the way through it;
- two areas never share a cell, and there is a wall between any two that are not joined;
- the open share reaches the target or the place fails, never neither;
- a recipe with one district and a recipe with three both build;
- opening loops never lengthens a walk, and with a threshold the place has at least one loop;
- the same seed builds the same place twice, and builds it the same after a pass is added to the chain;
- `run_retrying` returns the place attempt nought builds when attempt nought succeeds, and a failing first attempt followed by a passing second is the second's place on every run.

For the survey, on small maps drawn by hand: a dead end, a chokepoint, a junction of three openings, the loop count and the seclusion of a room off the route are each what the drawing says.

In `rl-rules`: the recipe file loads, and each mistake section 6 lists is refused by name.

In foundry: every deck builds within the retry bound on every seed of the range and meets its requirements, and every existing deck test passes unchanged.

A layout that meets every bound can still read badly, so sample decks are printed as text across seeds and looked at before the recipes are called done.

## 9. What is not here

- Prefabs attached by their own door, and a room refilled to a piece's size: the director's.
- Filling an area with what it was for: the furnisher's.
- Which pieces a run holds, limits per place and per run, states and rare events: the director's.
- Larger decks: a number in a file, raised when there is content to fill them.
- Caves and any shape that is not built of rectangles: a `Shape` a later slice adds.
