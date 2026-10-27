# Introduction

This guide builds a roguelike called Warren: a rat warren four floors deep, dark except for the lantern you carry, with a crack of daylight at the bottom that ends the run.

![Warren: a dug room lit by sight, remembered corridors in cold blue, two rats closing in, and the log counting their bites](images/warren.png)

Nine steps, each a complete program you can run, and the first six playable in this page.
Each step is the one before it plus something new:

1. A map, and walking on it.
2. Darkness, a lantern, and the first panels.
3. Rats, combat and a log.
4. Items to eat and throw, and rats that throw back.
5. An ability of your own.
6. Four floors, stairs, and a way to win.
7. Monsters and tile looks moved into files.
8. A new action, the shove.
9. A rail of panels down the side.

## Two rules the engine follows

**The engine owns the loop.**
It holds the turn order, field of view, combat and the map.
Your game supplies the decisions: what a tile looks like, what a monster wants, what eating a crust does.

**The engine never names your content.**
There is no `MonsterKind` enum or `Tile::Wall`.
Tiles, damage kinds, factions and abilities are ids in registries you fill.

## What you need

Rust, and Bevy's ECS: components, systems, resources, `Query` and `Commands`.
Warren draws with a glyph terminal the engine ships, so you need nothing from Bevy's renderer.

## Your own project

Add both crates to `Cargo.toml`:

```toml
[dependencies]
atglyph = { git = "https://github.com/rudehn/rl-engine", tag = "v0.5.0" }
bevy = "0.19"
```

List `bevy` yourself, at the version atglyph uses.
Bevy's derive macros, such as `#[derive(Component)]`, look for it in your own `Cargo.toml`.

Every step imports `atglyph::prelude::*`, plus the topic modules it needs, such as `atglyph::prelude::{combat::*, items::*}`.
