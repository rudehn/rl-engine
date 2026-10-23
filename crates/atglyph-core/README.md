# atglyph-core

Seeds, dice, geometry, ids and the turn queue for a turn-based roguelike (tier 0 of the atglyph engine).
It needs no Bevy, so it can be used on its own in any Rust project.
[`atglyph`](https://crates.io/crates/atglyph) is the whole engine, and a game depends on `atglyph` and reaches it as `atglyph::primitives`.
