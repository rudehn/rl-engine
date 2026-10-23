# atglyph-mapgen

Procedural map generation as a chain of seeded passes: rooms, BSP, cellular caves and prefabs (tier 1 of the atglyph engine).
It needs no Bevy, so it can be used on its own in any Rust project.
[`atglyph`](https://crates.io/crates/atglyph) is the whole engine, and a game depends on `atglyph` and reaches it as `atglyph::mapgen`.
