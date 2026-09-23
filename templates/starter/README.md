# {{project-name}}

A roguelike built on [rl-engine](https://github.com/rudehn/rl-engine), started from its template.

```sh
cargo run
cargo run -- --seed 7
cargo test
```

The first build compiles Bevy and takes a few minutes.

## What is here

All of it is in `src/main.rs`, in the order it happens.

- **A floor of rooms**, generated from the run's seed by a map-generation chain the engine runs the first time the floor is entered.
- **Sight and light.** The floor is dark. You see by the torch you carry and the braziers in the halls, and remember what you have seen.
- **Goblins** that wander, notice you by sight, sooner when you stand in light, and hunt where they last saw you.
- **Combat.** Walk into a goblin to strike it; armor takes its share of every blow. The engine narrates the fight, names things in their own colours, and ends each run on a screen that says how it went.
- **Stealth.** Put your torch out with `t` and a goblin has to be close to notice you, but you are nearly blind too.
- **Panels**: your health and the keys along the top, the log along the bottom, and a look cursor on `x` that forecasts a fight.
- **Tests** that run the game with no window: a property over forty-eight seeds, and two that play keys through the real input.

## Keys

| Key | What it does |
|---|---|
| arrows, `hjklyubn`, numpad | walk, or attack whoever is there |
| `.` | wait a turn |
| `t` | put the torch out, or light it again |
| `x` | look around; `tab` cycles, `esc` closes |
| `?` | every key this game answers to, listed from the same declarations |
| `esc` | the menu: a new run, the same seed again, or quit |
| `q` | quit |

## Your first change

Everything the floor is made of is a constant at the top of `src/main.rs`.
Change `GOBLINS` from 10 to 30 and run `cargo run -- --seed 7` twice.
The same seed always builds the same floor, so what changed is your change and not a new map.

From there:

- `BRAZIERS` and `TORCH` decide how much of the dark you can see into, and how far into it you can be seen.
- A new monster is one more definition beside the goblin's, and one more line where they are spawned.
- A new key is a field on `Keys`, a line in `declare_controls` and a branch in `player_input`. The `?` screen picks it up with no further help.

## Where to go next

The [guide](https://github.com/rudehn/rl-engine/tree/v0.3.0/docs/guide/src) builds a roguelike one chapter at a time, and each chapter is something to add here:

- things to pick up and use, chapter 6;
- stairs and more floors, chapter 7;
- monsters and items read from RON files, chapter 8;
- an action of your own, chapter 9;
- more panels, chapter 10;
- abilities, statuses and saving, in "Where to go next".

The engine's `delve` example is this template grown up: five floors, a brand that burns down, and abilities.
