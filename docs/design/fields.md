# Fire and gas

How the engine keeps a value per tile that changes turn by turn, and the two things built on it: gas, whose kinds are a game's content, and fire, whose rules are the engine's.
The reasoning is here; `docs/OVERVIEW.md` lists what exists.

## 1. One field type

`rl_grid::TileField<T>` is a grid of values stepped whole: a rule reads the field as it stood and writes every cell's next value into a second buffer, and the buffers swap.
Nothing a step changes can spread again within the same step, so a fire cannot race across a map in one turn and a cloud cannot cross a room because its cells were visited left to right.
The second buffer is kept, so a step allocates nothing.

Plan section 3.9 asked for exactly this, and for fire, gas, sound, scent, heat and blood to be one type with different rules.
Two rules ship now, and sound or scent is a third rule over the same type, not a second type.

## 2. Gas is content

A game names its gases in `Registries::gases`: smoke that hides what is behind it, a poison that bites above a concentration, a vapour that burns.
`GasDef` holds only what the engine acts on:

- `spread`: how freely it moves a turn, which drives both how it evens out and how it swirls.
- `fade`: the share of what is in a cell lost each turn, and never less than one unit, so every cloud clears.
- `veils_at`: the concentration at or above which it blocks sight.
- `burns`: whether fire catches in it.
- `inflicts`: a status for some turns on whoever breathes it at or above a concentration.

Everything else a gas does is the game's, answered from `Breathed`, sent every turn an actor stands in some.

A tile that stops a thrown thing stops gas too: walls and closed doors hold it back, and open water does not.

### Letting it go

Gas is let go at a point with an amount, and a cell holds at most `FULL`, 255.
What the cell has no room for spills outward at once, nearest cell first, filling each and passing through the ones already full, with a hashed nudge to which of two nearly as near cells fills first so the edge is ragged.
What no reachable cell has room for is lost.

This is what makes a grenade a cloud.
The first version added the amount to one cell and saturated at 255, so a smoke grenade was nine cells at 160 that thinned below sight-blocking in four turns: the gas that was asked for had nowhere to go.
Spilling puts the whole amount somewhere, and lets walls shape it, so the same grenade is a round cloud in the open and a long plume down a corridor, and a second one thrown into a cloud widens it rather than thickening it.
Because a cell never holds more than full, concentration stays a `u8` and saves did not change.

### Stepping it

A turn does three things to every cell at once, in `gas::diffuse`:

- An even exchange with every neighbour that holds gas, truncated toward zero across each edge so what one cell takes is exactly what the other gives.
- A swirl: each cell pushes a share of itself to one neighbour, the direction hashed per three-by-three patch and the share per cell, so a patch pushes one way together and moves a lobe of the cloud rather than speckling it.
- A clamp and a fade: what is left is held to no more than the densest of the cell and its neighbours before the turn, and then loses `fade` percent and at least one unit.

`spread` is split between the first two in a fixed ratio, sixty to forty, so a gas that spreads fast also churns and one that does not spread stays where it was let go.
The two shares together never move more than a cell holds, so no cell goes below nothing whatever the spread.

The clamp is what keeps the guarantee.
Two swirls meeting could pile gas into a cell denser than anything around it; held to the densest before the turn, the densest cell never gains, and with the fade it loses at least one unit a turn, so every cloud is gone within 255 turns at the very longest and in practice a grenade's in about forty.
The rolls are a `position_hash` of a seed derived for the turn, as fire's are, so a cloud spreads the same whichever order its cells are visited in and a save holds no generator for it.

The directions and shares are worked out once before the step and read by index, and whether each cell holds gas is asked once rather than by each of its eight neighbours.
Asked per neighbour, a step was twenty times the old exchange's cost on a field full of gas; worked out once, it is a little over one and a half times on a full field, and cheaper than the old exchange on a map with one cloud in it, since the old one asked the map about every neighbour of every cell whether or not there was gas near it.

### What was tried and not kept

- **An even exchange alone, with a random share per edge.**
  Spreading is averaging, and a random share per edge averages out within a turn: the clouds with and without it were near cell for cell the same.
- **Thinning from the edge**, a cell losing gas for each empty neighbour, so a cloud's core lasts and its edge wears away.
  It made a cloud's life depend on the room it was in, the same grenade gone in fifteen turns in the open and in sixty-seven in a sealed room, and gave no bound on how long a cloud lasts.
  How gas disperses is the same rule wherever it is.
- **A flat loss a turn in place of a percentage, written as how many turns a full cell lasts.**
  It gave a clean bound, but a cloud that shrinks in place, and a sealed room that clears on a single turn.
  A percentage leaves thin gas thinning slowly, so a cloud grows as it thins and leaves a haze behind after it stops hiding anything, which is what smoke does.
- **A setting of its own for the swirl.**
  One more number per gas for something `spread` already says: how freely it moves.
- **Draughts**, a direction each map pushes gas in.
  Left out; see below.

### Two gases

Each gas is its own field, and gases overlap without pushing each other aside: sight stops where any of them veils, whoever stands in two breathes both, and the map draws the densest.
Fire is the one thing that makes two meet, burning a vapour away and giving off smoke.
A cell holding at most full across every gas, so that smoke thrown into poison pushes it out, would couple every gas's step to every other's, and is left until a game wants it.

## 3. Fire is the engine's

A burning cell holds the turns it has left.
Each turn every burning cell burns down one turn, and every cell that has something to burn and is not burning may catch, with one roll against each burning neighbour.
What a cell can burn as comes from three places, and the most flammable wins:

- its tile, whose `TileProps::burn` gives a chance to catch, how long it burns, and the tile it leaves;
- an entity standing or lying on it with `Flammable`;
- a gas in it that `burns`.

A tile's `leaves` is required.
A tile that burned and stayed itself could catch again from the neighbour it just lit, and two such tiles would pass one fire between them forever; consuming the fuel is what makes every fire end.
For the same reason an entity that burns out loses `Flammable`, and burning gas is used up.

Rolls are a hash of the run's seed, the turn and the cell, not draws from a stream.
The spread is the same whichever order the cells are visited in, and a save needs no generator state to replay it.

A game decides what burning does to what burns:

- `FireRules::inflicts`: a status for some turns on whoever stands in fire.
- `FireEvent`: `Scorched` for an actor in fire, `Caught` and `BurntOut` for a flammable entity, `TileBurnt` for terrain that burned away. A crate that burns out is the game's to despawn, char or loot.
- `FireRules::smoke`: a gas each burning cell gives off.

`Burning` on an entity keeps its own cell alight while it lasts, forever for a fixture, so a lit brazier on dry ground sets the ground around it going.
`Kindle` asks for fire on a cell, and the `Ignite` ability effect asks for it on every cell of a footprint; a cell with nothing to burn still burns for the turns asked, which is a fireball scorching bare stone.

## 4. What the rest of the engine sees

- Sight: a gas at or above its `veils_at` is written into the map's veil, which `WorldMap::is_opaque` reads beside the tile. Field of view and light both read opacity, so smoke hides and shades alike, and the map's opacity epoch refreshes every viewshed when it moves.
- Light: burning cells glow through `Lighting::set_glow`, cast in the dynamic layer with the sources that move.
- Minds: a mind will not step into fire.
- Drawing: the map view draws flames over what burns and tints the ground under gas, from `FieldAppearance`, which a game recolours.
  It draws what `ShownFields` shows rather than the fields as they stand: a cell blends toward what its fields hold over a third of a second, so a cloud rolls on a turn rather than jumping, and a blast's front holds the blend back until it arrives, so a grenade is seen to go off and then leave its smoke and fire behind.
  The thickness a cloud shows is stirred by noise over the wall clock, so smoke that hangs still moves, and a cloud in sight is drawn whole over known ground, since one thick enough to hide behind hides its own far side and would otherwise read as a wall.
  None of it touches what the fields hold, so a headless game plays the same turns.
- Saving: `EngineSave` records every lit cell and every cell with gas, on every map.

## 5. Where it lives

One field per map, as places are kept: leaving a place freezes its smoke where it hangs.
On the streamed surface a field covers the loaded window and moves with it; what leaves the window is dropped, since a fire or a cloud nobody can see is not worth remembering.

Fire and gas are two plugins, each opt-in, because a game may want smoke and no fire or the reverse.
Both run in `ResolveSet::Fields`, after the turn's actions and before its ticks, so a status fire or gas puts on whoever stands in it lands and bites on the turn they stood there.
Fire runs before gas, so a fire that burns a vapour away and gives off smoke has the smoke spread on the same turn.

## 6. Not here

- Heat, cold and extinguishing. A cold aura putting out fire is a game's `Ignite` in reverse today, and a rule of its own when a game asks.
- Liquids that flow.
- Gas that moves with wind or draughts.
  Swirls and the shape of the rooms already make every cloud spread differently, and a draught is a second set of rules about where air goes, for a game that asks for it.
- Gases that displace each other; see above.
