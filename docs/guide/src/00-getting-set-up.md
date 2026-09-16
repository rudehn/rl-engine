# Getting set up

There are two ways in, and they answer different questions.
Generate a game from the template to start one of your own.
Clone the repository to follow the chapters after this, where every step is already written and you can run a chapter's result before you write it.

## Start a game of your own

```sh
cargo install cargo-generate
cargo generate --git https://github.com/rudehn/rl-engine --tag v0.2.0 templates/starter --name my-game
cd my-game
cargo run
```

It plays from the first build. The one file holds a floor of rooms, a torch and braziers in the dark, goblins that notice you by sight and hunt where they last saw you, walking into one to strike it, a status row, a log, a look cursor, and three tests that play it without a window.
Every chapter after this one is something to add to it.

The template pins the engine to the release named by `--tag`, and takes the template from that release too.
Leave the tag off and the template comes from `main`, which may use something no release has yet.
CI generates and builds the template on every change to the engine, so it does not rot.

## Follow the guide

```sh
git clone https://github.com/rudehn/rl-engine
cd rl-engine
cargo run -p tutorial --bin step01_a_map
```

Each chapter names the binary it builds, and they run in the same way.
The first build compiles Bevy and takes a few minutes.

## Add it to a project you already have

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

## If the build fails

On Linux, Bevy needs a few system packages that a Rust toolchain does not bring.
On Debian and Ubuntu:

```sh
sudo apt install pkg-config libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev
```

Fedora wants `alsa-lib-devel systemd-devel wayland-devel libxkbcommon-devel`, and Arch wants `alsa-lib systemd-libs wayland libxkbcommon`.
macOS and Windows need nothing beyond the toolchain.
If the build succeeds and the window is black, the game is drawing before the first floor exists; run it again with `RUST_LOG=warn` and read what the engine reports missing.

Next: [a map on screen](01-a-map-on-screen.md).
