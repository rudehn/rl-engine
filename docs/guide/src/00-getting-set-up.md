# Getting set up

## In the repository

The easiest way to follow along is to clone rl-engine and edit the tutorial crate in place.
Every step is already there, so you can run a chapter's result before writing it.

```sh
git clone https://github.com/rudehn/rl-engine
cd rl-engine
cargo run -p tutorial --bin step01_a_map
```

The first build compiles Bevy and takes a few minutes.

## From the template

To start a game of your own rather than follow along, generate one:

```sh
cargo install cargo-generate
cargo generate --git https://github.com/rudehn/rl-engine --tag v0.2.0 templates/starter --name my-game
cd my-game
cargo run
```

The template pins the engine to the release named by `--tag`, and `--tag` takes the template from that release too.
Leave it off and the template comes from `main`, which may use something that release does not have yet.

It is one file that runs from the first build: a floor of rooms, a torch and braziers in the dark, goblins that notice you by sight and hunt where they last saw you, walking into one to strike it, a status row, a log, a look cursor, and three tests that play it without a window.
Every chapter after this one is something to add to it.
CI generates and builds the template on every change to the engine, so it does not rot.

## In your own project

rl-engine is not on crates.io yet, so depend on it from git.
The `rl-engine` crate re-exports every other crate in the workspace.

```toml
[dependencies]
rl-engine = { git = "https://github.com/rudehn/rl-engine", tag = "v0.2.0" }
bevy = "0.19"
rand = { version = "0.9", features = ["std", "std_rng"] }
serde = { version = "1", features = ["derive"] }
```

Two imports open almost everything:

```rust
use bevy::prelude::*;
use rl_engine::prelude::*;
```

`Rect` is deliberately left out of the engine prelude, because Bevy has one of its own.
Import the grid one by name where you need it:

```rust
use rl_engine::rl_core::Rect;
```

## Tiers

| Tier | Crates | Bevy |
|---|---|---|
| 0 | `rl-core` | no |
| 1 | `rl-grid`, `rl-mapgen`, `rl-world`, `rl-rules` | no |
| 2 | `rl-bevy`, `rl-render`, `rl-ui`, `rl-overworld`, `rl-save` | yes |
| 3 | `rl-engine` | facade |

Map generation, field of view, pathfinding, the damage pipeline and the AI brains are tier 1.
They run headless, test in milliseconds and build for WebAssembly, which is what makes [chapter 11](11-testing.md) possible.
CI enforces the boundary.

A tool that only needs one of them can depend on that crate alone and never compile Bevy:

```toml
[dependencies]
rl-grid = { git = "https://github.com/rudehn/rl-engine" }
```
