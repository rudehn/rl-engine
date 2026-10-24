# Layout

Status: built 2026-10-01, against `main` at `235f864`; grown on for Undermere's dungeon from 2026-10-08, as section 10 lists.
The reasoning is here; `docs/OVERVIEW.md` lists what exists, and `docs/guide/src/systems/layout.md` is the reference.
The research behind it is `docs/research/gridsage-mapgen.md`, read for its concepts and not for its algorithms.

## 0. Summary

A place built by `Rooms` is rectangles scattered in rock and joined by whichever corridor reached the last one.
It reads as a dungeon, it is the same everywhere, and nothing about it can be asked a question: no pass knows which room is a dead end, which door is the only way through, or which corner is far from the way out.

This subsystem builds a place by accretion instead.
A first piece is laid down, and every later piece is attached to a wall of what already stands, with an opening between, so the place is connected by construction and its rooms share walls the way a built thing's do.
Which shapes are drawn depends on where on the map the piece is going, so a place has regions that differ, and after growth the longest detours are cut through so there is more than one way round.

Then the place is surveyed, and the survey is published: every area, every door, and for each what a later pass needs to decide what belongs there.
A place that fails what its recipe requires is thrown away and built again from the next attempt of the same seed.

It was the first of four pieces of work, and the other three key on what it publishes: a furnisher that fills an area to its size, a director that decides what a run holds and where, and tells that show a thing before it is reached.
The furnisher and the director are built, as the dressing and the pool of section 10; the tells are not.

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

7. Turn every passage with one way in back into what it was carved from, with the opening into it, and take growth up again from step 2 to refill the share, for a few rounds at most; the last round's trim stands, and a place then short of its share fails.

Every area is joined to one that was already joined, so every open cell can be walked to from every other, before any check is made.
The ring in step 4 is what keeps one wall between two areas and never none.

Step 7 was not in the first design.
The first decks printed had service passages ending at walls all over them, and a passage is the one area nothing is ever put in, so a dead end that is a passage can never pay; a dead end that is a room can.
Trimming before the place is handed on, and not as a later pass, is what keeps an area's index its place in the list for everything that reads it.
A game that wants a blind corridor draws it as a shape that is a room.

How full the place is grown and how many districts it has are a share and a density, never a count of cells of map.
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
The recipe also caps how many are opened, and no pair of areas gains two.
The threshold, the cap and the bounds a recipe requires are plain counts and not shares, so a recipe moved to a much larger map wants them raised with it.
A door only shortens a walk, so a detour measured before the last door is a ceiling on what it is now, and only the candidates whose ceiling could still be the longest are measured again; measuring every one after every door took seconds on a map of four hundred cells a side.

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

A loose generator is only safe if what it gets wrong is thrown away, and the module docs of `atglyph-mapgen` already say a caller retries a failed chain with a different index.
Nothing does: `Chain::run` draws every pass's stream at index nought, and every game propagates the error.
So `Chain::run_retrying(make, seed, attempts)` is added: it builds a fresh context with `make`, runs the chain with every pass's stream derived at the attempt's index, and returns the first that succeeds, or the last error once the attempts are spent.
The same seed therefore builds the same place however many attempts it took, and `Chain::run` is attempt nought.

## 6. The recipe file

A recipe is data, and its file is the engine's, as a prefab's is.
`atglyph-rules` owns the file and what is wrong with it, reading its barriers straight into `atglyph_mapgen::Barrier` and refusing one by the pass's own `Barrier::may_cut`, and `atglyph_rules::grow::passes` turns a loaded recipe into the passes, the way `PrefabLibrary::piece` turns a loaded prefab into an `atglyph_mapgen::Prefab`.
So a recipe loads, every mistake in it is reported by name, and a place is grown from it, with no `App`: a tool or a terminal game grows the places the framework does.

A layout file holds the shapes, the districts and the recipes:

- a shape: a name, and one of the three the engine ships with its ranges and how it is joined;
- a district: a name and `(shape, weight)` rows;
- a recipe: a name, a width and a height, the target open share, the run of failed attempts allowed, the district density, `(district, weight)` rows, the loop threshold and cap, and its requirements.

Checked at load: no name is used twice, every name resolves, every list has a weight more than nought, every range starts at one or more and has its smallest no larger than its largest, a cross's bars are thinner than they are long, a mouth has a width, every share is within a hundred, patience is more than nought, the requirements' shape names are shapes some district of the recipe can draw, and the recipe's size holds some shape it draws and every shape it requires, a rectangle the way up it is drawn, no weights overflow, no size, range or density is past any map, and no more loops are required than may be opened.
Whether a recipe's requirements can be met at all is not checked at load, since only growing a place can say; a game's test over a span of seeds is what finds a recipe that asks too much.
The file carries the top-of-file comment listing every field and every option.

A game with a `Shape` of its own builds its `Accrete` in code; naming one from a file waits for a game that has one.

## 7. Foundry

`assets/layout.ron` holds foundry's five shapes, three districts and two recipes.
The districts are halls, cells and service passages, and each recipe weights all three: `assembly`, which favors halls, for decks one to three, and `warren`, tighter and with more loops required, for the seven below.
Each deck names its recipe, so a deck's size is data and the `70, 40` that was in `decks.rs` is gone.

A deck starts at 84 by 48, about 1.4 times the area it has now.
Accretion wastes no rock between rooms, so that holds noticeably more than the scatter does, and it stays small while there is little to put in it.
The map view already follows the player and clamps to the map, so a deck larger than the screen needs nothing new.

The deck's chain swaps `Rooms` and `Doors` for `Accrete` and `OpenLoops`, and adds `Survey` after `FarthestExit` and `Require` last.
The armory, the store, the guard post, the reactor, the core and the furnishing stay as they are, stamped into the rooms accretion emits, until the furnisher and the director replace them.
`Foundry::build` runs the chain retrying, forty attempts at most, and `foundry --deck N --seed S` prints a deck as rows of glyphs, which is how the recipes were read before they were called done.
One test that ran a script on a fixed seed moved to another seed, and the fingerprint tripwire was re-baselined on a seed whose deck still brings a droid into sight.

## 8. Testing

In `atglyph-mapgen`, with no `App`, over a range of seeds and at two sizes, 84 by 48 and 160 by 90:

- every open cell of an accreted place can be walked to from every other;
- no opening has rock on either side of the way through it;
- two areas never share a cell, and there is a wall between any two that are not joined;
- the open share reaches the target or the place fails, never neither;
- a recipe with one district and a recipe with three both build;
- opening loops never lengthens a walk, and with a threshold the place has at least one loop;
- the same seed builds the same place twice, and builds it the same after a pass is added to the chain;
- `run_retrying` returns the place attempt nought builds when attempt nought succeeds, and a failing first attempt followed by a passing second is the second's place on every run.

For the survey, on small maps drawn by hand: a dead end, a chokepoint, a junction of three openings, the loop count and the seclusion of a room off the route are each what the drawing says.

In `atglyph-rules`: the recipe file loads, and each mistake section 6 lists is refused by name.

In foundry: every deck builds within the retry bound on every seed of the range and meets its requirements, and every deck test that was there before passes unchanged.

In `atglyph-mapgen` again, since the trim: no passage of a grown place is left with one way in, and areas are numbered in the order they are emitted.

A layout that meets every bound can still read badly, so sample decks are printed as text across seeds and looked at before the recipes are called done.

## 9. What is not here

- States and rare events: the director's, still to come.
- Larger decks: a number in a file, raised when there is content to fill them.

## 10. What grew since

Undermere's dungeon, from 2026-10-08, asked for more than rectangles and doors, and each addition is a pass or a field a chain that does not ask for it never sees, so no earlier place moved.
The module docs of `atglyph-mapgen` and `atglyph-rules` are the reference; the reasons are here.

- **A cave is a shape.** `Cave` draws a piece by the cellular automaton inside a box and keeps its largest region, so `Accrete` lays, joins and surveys it as it does a rectangle, and a place mixes caves and halls with no second generator.
- **Barriers.** Rock across a share of the map that growth goes round, laid at a chance and shifted as a whole, so a floor has a wall to walk round without a hand-drawn piece; a barrier some shift could lay from one edge to the other is refused at load, since it would part the place.
- **A drawn piece as an area.** A `PrefabShape` is a prefab grown as an area of its own, painted rather than carved and entered only at the entries its file draws, so a vault keeps its shape; joined `Locked` to a tag or `Sealed`, it grows nothing past its spare entries, so a lock keeps the piece alone, and the survey, the loops and the exit never walk through it.
- **A landmark.** A `FirstPiece` is laid before anything grows and the place grows out from it, so what a floor is remembered by stands where its entry says rather than wherever growth left room, and the way in and the way down may keep to opposite `Sides` of it.
- **Lakes and bridges.** By default a lake is a pond that never parts the floor; with `cut` one may, since ponds alone never make an island, and `Bridges` joins every part again by a straight crossing or fails the place for `run_retrying`.
- **Secrets.** `HideOpenings` walls over a dead end's only door or a loop's shortcut and publishes the cell as a `SecretCell`, never one the way down needs, so a hidden room is always optional; a secret is a record on its place's map rather than an entity, since a wall nobody has found is the map's, not a thing standing on it.
- **The pool, which is the director.** A run's pieces are drawn once, at its start, from one pool file, by floor, tag, weight, limits per place and per run, a group and a budget per kind, so no limit is passed whichever floor is built first and a floor grown again after a failed attempt is another layout holding the same pieces; `StampPicks` lays each in the area it prefers, from what the survey published.
- **The dressing, which is the furnisher.** A dressing is a legend and rules for where each glyph goes, along walls, in corners, in rows or scattered, rather than rows of a fixed piece, so one dressing fills a closet, a hall and a cave; `Dress` works on an area's own cells, never beside an opening, and takes back anything that would cut an area in two or shut a slot in.
- **Walking through a shut door.** The survey, the loops and the farthest exit walk every cell the registry marks passable, a shut door included, and never through a lock, where they had read `walkable` and so stopped at the first closed door.
- **Tier 1 throughout.** Reading the files and turning them into passes is `atglyph_rules::grow`, over `atglyph-mapgen`'s own types, so a place is grown from its files with no `App` and no copy of a mapgen type to keep in step.
