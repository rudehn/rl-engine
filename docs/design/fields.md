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

- `spread`: the share of each difference to a neighbour that flows across it per turn.
- `fade`: the share of what is in a cell lost each turn, and never less than one unit, so every cloud clears.
- `veils_at`: the concentration at or above which it blocks sight.
- `burns`: whether fire catches in it.
- `inflicts`: a status for some turns on whoever breathes it at or above a concentration.

Everything else a gas does is the game's, answered from `Breathed`, sent every turn an actor stands in some.

Concentration is a `u8`.
Spreading is an exchange between neighbours, so a cloud's total only falls, and the densest cell never gains, so a cloud always clears.
A tile that stops a thrown thing stops gas too: walls and closed doors hold it back, and open water does not.

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
- Gas that moves with wind.
