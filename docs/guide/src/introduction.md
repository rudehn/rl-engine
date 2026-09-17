# Introduction

This guide builds a roguelike called Warren: a rat warren under a granary, two floors deep, dark except for the lantern you carry, with a way out at the bottom that ends the run.

![Warren: a dug room lit by sight, remembered corridors in cold blue, two rats closing in, and the log counting their bites](images/warren.png)

Six steps, each a complete program you can run, and each one playable in this page without installing anything.
CI compiles every one of them, so the code in these pages is code that builds.

Each step is the one before it plus a single new thing, and the sixth is a small game with floors, fighting, items, an ability and a way to win.

## Two rules the API follows

**Own the loop or leave it out.**
The engine holds the turn loop, the scheduler, field of view, the occupancy index, the damage pipeline and the map.
Your game supplies the decisions: what a tile looks like, what a monster wants, what eating a crust means.

**The engine never names your content.**
There is no `enum MonsterKind`, no `Tile::Wall`, no `DamageType::Fire`.
Tiles, damage kinds, factions, statuses, item tags and abilities are opaque ids in registries you fill.

## What you need first

Rust, and Bevy's ECS: components, systems, resources, `Query`, `Commands`.
Not Bevy's renderer, assets or scenes.
Warren draws with a glyph terminal the engine ships.

To build and run the steps yourself, or to start a game of your own from the template, the project's readme has the three commands for each.

## How the chapters work

Each chapter takes the previous step and adds one thing.
Code in the text is pulled from the step's source, and the full file is linked at the top of every chapter.

Panels arrive when there is something to put in them rather than all at once, and the last chapter says where the rest of the engine is.

Next: [a map, and walking on it](01-a-map-and-walking.md).
