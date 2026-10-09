# Charts: what someone other than the player has seen

Written 2026-10-02, and built the same day.

## 1. What it is for

A mind that explores: it walks to ground it has not seen, and stops when there is none.
A rival party in a dungeon, a scout sent ahead, a companion that maps while the player rests.
And knowledge that is somebody's: what one party saw can be handed to the next, and what one side knows the other does not.

## 2. What was in the way

`Knowledge` is what the player has seen, one record, written by whoever carries `RevealsMap` and read by the map view.
A mind had sight and no memory of it: `Wander` drifts, `SearchLastKnown` walks to one cell, and nothing remembered where a mind had been.

## 3. The design

### 3.1 A chart is a set of tiles under an id the game numbers

`Charts` is a resource of tile sets keyed by `ChartId` and map.
A `ChartId` is a number the game chooses, and the engine never asks what it stands for: one scout, a party, a side.
Minds that name one chart share it, so what one has seen none of them goes to look at, and two explorers finish a floor sooner than one.

Declined: keying a chart by faction.
A faction is a combat idea, and a game with no combat has explorers too.
Two parties of one faction that know different things is also the first thing a game wants.

Declined: a chart per entity, as a component.
It dies with the entity, and the point of a leaked map is that it outlives whoever drew it.

`Knowledge` and `Charts` hold the same thing, a `TileSet`, the bucketed bit grids `Knowledge` always kept, factored out so the two cannot drift.
`Knowledge` keeps its regions and sites, which are the surface's and the player's alone.

### 3.2 Written on the mind's own turn

`chart_sight` runs in `DecideSet::Sense`, chained after the cast, and writes what the mind holding the turn sees into the chart its `Charting` names.
On its turn, not once a frame, because dozens of minds move within one frame and each must be told what it lacks from where it stands now.

### 3.3 What is lacking is a sense, and walking to it is a tactic

`sense_uncharted` pushes `Uncharted`, the cells of the loaded window the chart does not hold that could be stood on, each beside a held cell that could be stood on.
`Explore`, in `rl-rules`, asks the way toward all of them at once and takes a step, so the nearest by the way there wins, not the nearest by the crow.
It gives the turn up when the list is empty, or when no cell of it can be reached, so an explorer with nothing left to find goes on to whatever its brain does next.

The unseen cells are asked whether they can be stood on, which is a small thing the mind has not earned: it knows which dark cell beside known floor is floor.
The alternative is to walk to the known cell and look, which strands an explorer on a cell whose unseen neighbour is a wall it cannot see round a corner, forever one step from a goal that is not there.
A cell is a goal only beside known ground by a straight step, so it is always one step from somewhere the mind could already stand.

The way there is found over the map as it is, not the map as charted, so a route may cross ground the mind has not seen.
That is the same licence every other tactic has, and a game that wants a mind to path only through what it knows has the chart to build its own field from.

### 3.4 Handing a chart on

`Charts::copy(from, into)` adds what one chart holds to another and leaves the first as it was.
`Charts::forget` empties one.
Both are the game's to call: the engine never decides that someone told someone.

### 3.5 Saved

`EngineSave` carries every chart, empty by default, so a save written before charts loads.
`Charting` is a component on the game's own entity, and the game saves it with the rest of that entity.

## 4. What waits

- A chart of things as well as tiles: where a prop or an item was last seen. Today a game keeps that itself.
- The player's own knowledge as a chart, so the map view could draw any chart. `Knowledge` is still its own resource.
- Charting by something that is not a mind.
