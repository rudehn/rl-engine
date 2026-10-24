# atglyph-grid

Terrain, field of view, pathfinding, Dijkstra maps and a spatial index for grid-based games (tier 1 of the atglyph engine).
It needs no Bevy, so it can be used on its own in any Rust project.
[`atglyph`](https://crates.io/crates/atglyph) is the whole engine, and a game depends on `atglyph` and reaches it as `atglyph::grid`.

Used alone, it needs the crates it builds on at the same version, because its examples import from them:

```toml
[dependencies]
atglyph-core = "0.4.0"
atglyph-grid = "0.4.0"
```
