# atglyph-mapgen

Procedural map generation as a chain of seeded passes: rooms, BSP, cellular caves and prefabs (tier 1 of the atglyph engine).
It needs no Bevy, so it can be used on its own in any Rust project.
[`atglyph`](https://github.com/rudehn/rl-engine) is the whole engine, and a game depends on `atglyph` and reaches it as `atglyph::mapgen`.

Used alone, it needs the crates it builds on at the same tag, because its examples import from them:

```toml
[dependencies]
atglyph-core = { git = "https://github.com/rudehn/rl-engine", tag = "v0.4.0" }
atglyph-grid = { git = "https://github.com/rudehn/rl-engine", tag = "v0.4.0" }
atglyph-mapgen = { git = "https://github.com/rudehn/rl-engine", tag = "v0.4.0" }
```
