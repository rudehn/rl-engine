<!-- documents:
     plugins: FirePlugin, GasPlugin
     files: crates/rl-core/src/seed.rs
            crates/rl-grid/src/field.rs
            crates/rl-grid/src/tile.rs
            crates/rl-rules/src/fire.rs
            crates/rl-rules/src/gas.rs
            crates/rl-bevy/src/fields.rs
            crates/rl-bevy/src/fire.rs
            crates/rl-bevy/src/gas.rs
            crates/rl-bevy/src/effects.rs
            crates/rl-bevy/src/world.rs
            crates/rl-bevy/src/plugin.rs
     fingerprint: 5df82794 -->

# Fire and gas

A field is one value per tile, stepped a whole turn at a time: a rule reads the grid as it stood and writes each cell's next value into a second buffer, and the two swap.
Fire is a field of the turns each burning cell has left, spread by rules the engine owns through whatever a cell has to burn.
Gas is one such field per registered gas, holding a concentration, spread and faded by numbers a game's content names.
Both are kept per map, so smoke left hanging in a corridor is hanging there still on the way back, and both are stepped inside the turn that caused them.

## Turning it on

`FirePlugin` inserts `Fire` and steps it in `FieldSet::Fire`; `GasPlugin` inserts `Gases` and steps it in `FieldSet::Gas`.
Both of those sit inside `ResolveSet::Fields`, after the turn's actions and before what ticks because a turn passed, so a status the flames or a cloud put on whoever stood in them lands and bites on the turn they stood there.
Fire runs before gas, so a fire that burns a vapour away and gives off smoke has that smoke spread on the same turn.
Each is opt-in on its own, because a game may want smoke and no fire, or fire and nothing in the air.
`FirePlugin` declares `needs::<FireRules>`, `needs::<Registries>` for which gases burn, and `needs::<Seed>` for the rolls; `GasPlugin` declares `needs::<Registries>` for the gases themselves.
Both check again as play begins that what the content asks for can happen: fire that inflicts a status wants `StatusPlugin`, fire that smokes wants `GasPlugin`, and a gas that inflicts a status wants `StatusPlugin`, each refused by name rather than left to do nothing.
Adding `FirePlugin` registers the `Ignite` ability effect, which writes a `Kindle` for every cell of a footprint, and `GasPlugin` registers `Emit`, which writes a `Release`, so an ability reaches either field without knowing there is a field.
Both reset on a new run, and both declare `depends_on::<CorePlugin>`.

## The model

`TileField<T>` is the field: `step` asks a rule for every cell's next value through an `Around` that reads the grid as it stood, and then the buffers swap.
Nothing a step changes can move again within the same step, so a fire cannot run across a map in one turn and a cloud spreads the same way whichever order its cells were visited in; the second buffer is kept, so a step allocates nothing.
`MapFields<T>` holds one per map, follows the map readers read, sets a field aside when play crosses to another and takes it up again on return, and on the streamed surface reframes with the window and drops what leaves it.
`Fire` is a `MapFields<u8>` of turns left, read through `is_burning`, `turns_at` and `burning`.
`Kindle { at, turns }` asks for fire on a cell; a cell with nothing to burn still burns that long and goes out, which is a fireball scorching bare stone, and a tile that both refuses a thrown thing and does not burn refuses it.
`Flammable { catch_pct, turns }` is what a game puts on a crate or a corpse, and `Burning` is what is alight, `Burning::forever` for a brazier that keeps its own cell burning for good.
What a cell burns as comes from three places and the most flammable wins: the `Kindling` its tile declares through `TileProps::burns`, a `Flammable` entity standing on it, and a gas in it whose `GasDef::burns` is set.
`fire::spread` burns every alight cell down one turn and rolls each unlit one against `catch_chance`, which compounds a chance for each of its eight neighbours that is burning.
The rolls are a `position_hash` of a seed derived for fire over the turn number, not draws from a stream, so the same cell rolls the same whatever order the step visited it in and a save carries no generator for it.
`FireRules` is the game's half of fire: `inflicts` a status on whoever stands in flames, `smoke` a gas each burning cell gives off, and `glow` the light it sheds, `FIRE_GLOW` unless a game says otherwise.
`FireEvent` reports `Scorched`, `Caught`, `BurntOut` and `TileBurnt`.
`Gases` holds one `MapFields<u8>` of concentration per registered gas, read through `at`, `densest` and `cells`.
A `GasDef` carries `spread`, `fade`, `veils_at`, `burns` and `inflicts`, and nothing else.
`gas::diffuse` exchanges a share of each difference with every neighbour that can hold gas, so the densest cell never gains, and then takes `fade` percent and never less than one unit, which is what ends every cloud whatever its shape.
A tile that stops a thrown thing stops gas too, so walls and closed doors hold a cloud back and open water does not.
`Release` asks for gas on a cell and `Vents { gas, amount }` gives some off wherever its entity stands, every whole turn.
Gas at or above its `veils_at` is written into the map's veil with `set_veil`, and `WorldMap::is_opaque` reads that veil beside the tile's own opacity, so sight and light both stop in thick smoke.
`Breathed` is sent for every actor standing in any gas at the end of a turn, whether or not the gas does anything to it.

## Using it

Gases are content, and a game names them in `Registries::gases` with the five fields the engine acts on: two rates, a threshold, whether it catches, and what breathing it does; Delve names two.

<!-- include: ../../../../examples/delve/src/main.rs:gases -->
```rust,no_run
    let gases = Registry::from_defs(vec![
        // What burning flesh gives off: thick enough to hide in while it hangs.
        GasDef::new("smoke").spread(55).fade(9).veils_at(70),
        // The reek off a pool of bile. It burns, and a lungful makes the head swim.
        GasDef::new("reek").spread(35).fade(12).burns().inflicts(60, statuses.expect("dazed"), 1),
    ])
    .unwrap();
```

Fire takes one resource before play begins, saying what its flames do beyond burning, and Delve's is one line.

<!-- include: ../../../../examples/delve/src/main.rs:fire_rules -->
```rust,no_run
    // Standing in fire scorches, and burning flesh smokes.
    commands.insert_resource(FireRules::new().inflicts(registries.statuses.expect("scorched"), 3).smoke(registries.gases.expect("smoke"), 30));
```

## The line

Whether anything burns at all is the game's: no tile burns unless its `TileProps` says how, and nothing else catches without a `Flammable` on it.
A tile that burns must name the tile it leaves, because a tile that burned and stayed itself could catch again from the neighbour it had lit a moment before, and two of them would pass one fire back and forth forever.
Using the fuel up is what makes every fire end, which is why the engine also takes `Flammable` off whatever burnt out and burns a vapour away where it caught.
What is left of a burnt crate is the game's answer: the engine writes `FireEvent::BurntOut` and leaves the entity standing, to be despawned, charred or looted.
The engine decides how fire catches and how gas moves; a game decides which gases exist, what each does beyond its `Breath`, and what standing in flames costs.
A concentration is a `u8` and a burning cell's clock is a `u8` of whole turns, so neither field anywhere carries a float.
Every burning cell is marked a hazard for the mind holding the turn, which is the one opinion the engine has about what a field means to somebody deciding where to step.
Both fields are the engine's to save: each exports the cells that hold something on every map, and a restored field is laid out again over whatever window each map has when it is next followed.

## Where it lives

`rl-grid` is tier 1 and has no Bevy in it: `field.rs` is `TileField` and the double-buffered step, over which the rule that nothing moves twice in a step is tested on a five-cell grid, and `tile.rs` is where a tile declares how it burns and refuses by name a tile it would leave that nobody registered.
`rl-rules` is tier 1 too, and holds both rules as functions over a borrowed field: `fire::spread` takes its tinder and its rolls as closures, and `gas::diffuse` takes a `GasDef` and a test for what holds gas.
Neither needs an `App`, which is why the properties they exist for are proved over seed ranges rather than watched: that a cloud of any shape clears, and that a firebreak holds whatever the rolls.
`rl-bevy` is tier 2 and owns where it burns: `fields.rs` keeps a field per map and per window, `fire.rs` and `gas.rs` are the two plugins with their components, messages and content checks, `effects.rs` has the two ability effects, and `plugin.rs` fixes the order of `ResolveSet::Fields`.
