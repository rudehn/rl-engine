# atglyph-grid

Terrain, field of view, pathfinding, Dijkstra maps and a spatial index for grid-based games (tier 1 of the atglyph engine).
It needs no Bevy, so it can be used on its own in any Rust project.
[`atglyph`](https://github.com/rudehn/rl-engine) is the whole engine, and a game depends on `atglyph` and reaches it as `atglyph::grid`.

Used alone, it needs the crates it builds on at the same tag, because its examples import from them:

```toml
[dependencies]
atglyph-core = { git = "https://github.com/rudehn/rl-engine", tag = "v0.5.0" }
atglyph-grid = { git = "https://github.com/rudehn/rl-engine", tag = "v0.5.0" }
```
