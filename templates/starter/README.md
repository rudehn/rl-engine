# {{project-name}}

A roguelike built on [atglyph](https://github.com/rudehn/rl-engine), started from its template.

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
- **Items.** Three potions, a dagger and a leather coat lie about the floor. Pick one up with `g`, drink a potion with `d` to mend, and put on or wield what you carry with `w`: the dagger strikes harder than your fists, and the coat's armor is added to yours on every blow.
- **Panels**: your health and the keys along the top, the log along the bottom, and a look cursor on `x` that forecasts a fight.
- **Tests** that run the game with no window: a property over forty-eight seeds, and tests that play keys through the real input, a blow, the torch, a potion and the coat.

## Keys

| Key | What it does |
|---|---|
| arrows, `hjklyubn`, numpad | walk, or attack whoever is there |
| `.` | wait a turn |
| `g`, `,` | pick up what is here |
| `d` | drink a potion |
| `w` | put on or wield what you carry |
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
- `POTIONS` and `POTION_HEALS` decide how forgiving the floor is.
- A new monster is one more definition beside the goblin's, and one more line where they are spawned; a new item is the same beside the coat's.
- A new key is a field on `Binds`, a line in `declare_controls` and a branch in `player_input`. The `?` screen picks it up with no further help.

## Where to go next

The [guide](https://rudehn.github.io/rl-engine/) builds a roguelike one chapter at a time, and each chapter is something to add here:

- things to throw, and monsters that pick things up, in [Things to carry, throw and eat](https://rudehn.github.io/rl-engine/04-things-and-minds.html);
- an ability, in [A knack of your own](https://rudehn.github.io/rl-engine/05-a-knack.html);
- stairs and more floors, in [Two floors, and a way out](https://rudehn.github.io/rl-engine/06-two-floors.html);
- monsters and items read from RON files, in [Content in files](https://rudehn.github.io/rl-engine/07-content-in-files.html);
- an action of your own, in [An action of your own](https://rudehn.github.io/rl-engine/08-an-action-of-your-own.html);
- more panels, statuses and saving, in [Where to go next](https://rudehn.github.io/rl-engine/09-where-to-go-next.html).

Each system the engine has, items and equipment among them, has a [reference page](https://rudehn.github.io/rl-engine/systems/items.html) of its own.

[Undermere](https://github.com/rudehn/undermere), a sister project of the engine's, is a whole game grown from the same start: ten floors under a lakeside town, a lantern that dims as you descend, and abilities.
