# rl-engine

A reusable engine for turn-based grid roguelikes, in Rust, on Bevy.

The engine is a workspace of small crates in tiers.
Tiers 0 and 1 know nothing about Bevy and are what make generation, pathfinding and rules testable in milliseconds and benchmarkable without a window.
Tier 2 is the Bevy layer: plugins, the turn loop, rendering, UI.
`scripts/check-tiers.sh` fails the build if a tier-1 crate ever grows a Bevy dependency.

| Tier | Crate | What it holds |
|---|---|---|
| 0 | `rl-core` | grid, geometry, seeds and streams, dice, typed ids, the turn queue |
| 1 | `rl-grid` | tile registry, terrain, FOV, A*, Dijkstra maps, spatial index, regions |
| 1 | `rl-mapgen` | the pass pipeline and the builders that run in it |
| 1 | `rl-world` | world graph, hydrology, sites, roads, chunk generation |
| 1 | `rl-content` | registries and banded tables loaded from RON |
| 1 | `rl-rules` | stats and modifiers, damage stages, statuses, factions |
| 1 | `rl-events` | facts, named counters, quests as objectives over facts |
| 1 | `rl-ai` | movement profiles, snapshots, tactic-priority brains over Dijkstra maps |
| 1 | `rl-test-support` | fixtures and property helpers |
| 2 | `rl-bevy` | plugins, components, system sets, the turn loop, chunk streaming, items, places |
| 2 | `rl-render` | the glyph grid renderer and the world view |
| 2 | `rl-overworld` | opt-in overworld screen with a portal picker |
| 2 | `rl-ui` | theme tokens, widgets, key hints, the log view |
| 3 | `rl-engine` | facade and prelude |

`docs/PLAN.md` is the design: what was decided, why, and which milestone each piece lands in.
`docs/reviews/` holds the code reviews of the three repos the engine was extracted from, with `path:line` citations for every claim in the plan.

## The worked example

`examples/corsair` is a small pirate roguelike built only on the public API: islands from the world graph, ports with huts, a bestiary and an armory in RON, factions, bump-to-attack combat, loot on the sand and in the pockets of the dead, a sea chest to wear it from, smugglers' caves under the coves with a treasure vault at the bottom, a ledger of tasks from RON that ends in a victory, a message log and the world map with a portal picker that works from anywhere.
It is what a game on this engine looks like.

```sh
cargo run -p corsair -- --seed 7
```

Keys: arrows, `hjklyubn` or the numpad to walk, `.` to wait, `g` to pick up, `>` `<` or Enter to use a cave mouth or stairs, `i` for the sea chest, `t` for the ledger of tasks, `m` for the map, `q` to quit.

## Principles

- **Own the loop or leave it out.** A struct plus a `SystemSet` marker is not a subsystem.
- **No theme in the engine.** Content is an opaque id in a registry the game fills. No `#[non_exhaustive]` enums with a `Custom` variant, and no closed taxonomy enums.
- **Traits as parameters.** `impl Rng`, `impl CostSource`, `impl OpacitySource`, `impl Fn(Id) -> bool`. Asking the caller for a capability is how a Bevy dependency disappears.
- **Determinism within a build.** One `RunSeed`, named domains, per-pass streams, no hash containers in gameplay paths.
- **Measure the hot paths.** Every algorithm crate carries criterion benches on realistic maps.

## Building

```sh
cargo test -p rl-core -p rl-grid      # tier 0 and 1, seconds
cargo bench -p rl-grid                # FOV, A*, Dijkstra, regions
scripts/check-tiers.sh                # the tier boundary
cargo test --workspace                # everything, builds Bevy
```

Dual-licensed under MIT or Apache-2.0.
