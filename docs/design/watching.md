# Watching: a run somebody watches and nobody plays

Written 2026-10-02.
Section 3 is the design; section 4 is the order it was built in.

## 1. What it is for

A game in which every actor is a mind: two sides fight, and the person at the keyboard set the board up beforehand and now watches.
An auto-battler, a simulation, an attract mode on a title screen, and a headless balance run of a thousand fights are all this.

## 2. What was in the way

The engine waits on one thing, the player holding a turn, and reads one thing to decide what is shown, the player's viewshed.
With no player at all:

- `run_turns` has nothing to stop for, so it runs to its ceiling of 512 passes every frame and a fight is over before it is seen.
- The map view draws only what the player's viewshed holds, so it draws nothing.
- The map being read follows the player through a warp and nobody else, so no place is ever entered, built into, or populated.
- The lists of what is in sight, the narrator and the particles all ask the player's viewshed, so nothing is listed, told or played.

## 3. The design

### 3.1 An onlooker is a player that is no actor

Decision: nothing new is added for who is watching.
A `Player` without `Actor` is never admitted to the queue, so it is never dealt a turn, and everything that asks what the player sees still has a player to ask.
The map being read follows it through a warp, `PlaceEntered` fires on its arrival, the map view centres on it, and the window streams round it.
Its `Position` is therefore the camera: a game scrolls a map larger than the view by moving the onlooker.

Declined: a `Vantage` system parameter that every reader of the player's viewshed would go through, answering from an all-seeing resource when there is one.
It touches the map view, the sighted list, the narrator, the particles and the prop spotting, and gives a second answer to "who is watching" that each of them could get wrong separately.
The onlooker changes none of them.

### 3.2 A viewshed that sees everywhere

`Viewshed::everywhere()` is a viewshed whose cast sets both bits of every tile of the loaded window: no wall stops it and no dark hides anything from it.
It is cast by the same `fov::cast` as any other, so it follows the window and the map, and with `RevealsMap` it remembers the whole map.
An onlooker carries one; an onlooker with an ordinary viewshed is a camera with a fog of its own, which is also a fair thing to want.

The flag is private to `Viewshed` rather than a component beside it, so `cast` keeps its signature and nothing that already holds a `Viewshed` has a second thing to query.

### 3.3 A pace

`Pace` is a `CorePlugin` resource: game time allowed per second of the wall clock, in hundredths of a step, or unpaced.
`earn_pace` adds the frame's share once a frame, and `run_turns` spends it as the turn clock advances and stops dealing when none is left.

- Unpaced is the default and is exactly the old behaviour, so no game changes a line.
- Passes that do not move the clock are free: everyone due at one reading acts in one frame.
- A pass that jumps the clock further than the credit in hand is paid off over the frames after it, so the average is the rate whatever the costs of the actions.
- A frame earns at most a quarter of a second, and idle credit never grows past that, so a hitch or a quiet stretch is not answered by a burst of turns nobody saw.
- A spent pace stops turns being dealt, in `schedule`, and not passes being run: every frame still runs one, so a player's intent is resolved in the frame it was written, no actor is left holding a turn across frames, and a warp or a reaction asked for while the turns are stopped is still answered. The first cut skipped the passes themselves, and a game that stopped the pace before its first frame never had its opening warp resolved.
- `Pace::stopped()` deals nothing, which is a pause.

The pace decides how many passes a frame runs and never what a pass does, so one seed plays one run at any pace.
A headless balance run leaves it unpaced and gets 512 passes a frame.

Declined: a fixed number of passes per frame.
It ties the speed of a fight to the frame rate, and a pass is not a unit of game time: twenty actors due at one reading are twenty passes and no time at all.

The clock stays an integer.
Credit is kept in millionths of a hundredth of a step, so a frame of a few milliseconds earns a whole number and no float ever touches the turn order.

### 3.4 Cues

`ParticlesPlugin` already holds the turns while a flight or a burst plays whenever the player holds no turn, which an onlooker never does.
So a watched fight is paced twice, by the pace and by what is worth seeing, and a game that wants to fast-forward sets `ParticleStyle` to instant as it would for anything else.

## 4. Order of work

Built on 2026-10-02 in this order, each with its tests.

1. `BitGrid::fill`.
2. `Viewshed::everywhere` and the cast.
3. `Pace`, `earn_pace` and the loop.
4. An onlooker through a warp into a place, and two sides of minds fighting to the end under one, at two paces with one outcome.
5. The map view under an onlooker, drawn whole.
6. The documentation pass: the turn loop and sight pages, the overview, the changelog and the README's feature list.

## 5. Risks

- **Replay.** A recording presses its keys when the player holds a turn, which an onlooker never does, so a watched run cannot be recorded by keys. Such a run is its seed and whatever was set up before it, which a game writes down itself.
- **Hidden props.** A hidden prop is spotted by the player's roll and is hidden from everyone until then. An onlooker that sees everywhere will spot them, which shows them to every mind as well. A game that wants things known to one side and not another needs knowledge per side, which is its own piece of work.
- **Panels that describe the player.** The vitals strip and the inspect forecast read the player's health and arms, which an onlooker has none of. They are expected to read as empty, and that is to be checked panel by panel when a game first puts one beside an onlooker.
