# Agent guide

`CLAUDE.md` is a symlink to this file. Edit this one.

## What this is

`rl-engine` is a multi-crate roguelike engine. Read `README.md` for the crate tiers and `docs/PLAN.md` for every design decision and the milestone plan.
The plan was written against four code reviews in `docs/reviews/`; when a decision looks odd, the reason is cited there.

## Rules that the build enforces

- Tier 0 and 1 crates (`rl-core`, `rl-grid`, `rl-mapgen`, `rl-world`, `rl-test-support`) must not depend on Bevy. `scripts/check-tiers.sh` checks it; CI runs it.
- `#![deny(missing_docs)]` on every crate. Doc-tests compile and run; never fence an example as `ignore`.
- `cargo fmt --all --check` must pass; `rustfmt.toml` pins the width, so do not hand-wrap.
- `cargo clippy --workspace --all-targets -- -D warnings` must pass. Do not add crate-wide `#![allow(clippy::too_many_arguments)]`; a system with sixteen parameters is a system to split.
- Tier-1 crates build on `wasm32-unknown-unknown`. No `std::time::Instant` in them.

## Rules that only review enforces

- **Own the loop or leave it out.** The previous engine shipped data structures and left the behaviour in the game, and the game copied the data structures instead of depending on it.
- **No theme words in engine crates.** No fantasy, sci-fi or pirate vocabulary in types, docs or constants. Content is an id in a registry.
- **No `#[non_exhaustive]` + `Custom { id }`** and no closed taxonomy enums for content. Registries and traits are the only extension mechanisms.
- **Randomness** comes from `rand`, through `RunSeed::derive(domain, index)`. Functions take `&mut impl Rng`. Never construct a generator from a constant or from entropy inside engine code.
- **No `HashMap` or `HashSet` in gameplay or generation paths.** `BTreeMap`, `Vec`, or a `BitGrid`, with the reason stated.
- **Costs and clocks are integers.** Hundredths of a step, the same unit everywhere.
- **Doc comments say why and why-not**, at the density of `rl-core/src/turn.rs`, not more.
- **Tests**: property-over-seed-range where a property exists, fingerprint tripwires labelled as such where none does, and a name that reads as a sentence describing the property.
- **Every RON schema** carries a top-of-file comment listing the full option space.
- **No `TODO` comments in source.** Outstanding work is tracked in `docs/PLAN.md` or an issue.
- Prose style: plain dash, never an em dash; American spelling in identifiers.

## Layout

```
crates/<name>/src/lib.rs   module-level //! docs name every public item
crates/<name>/benches/     criterion, on realistic maps, only for hot paths
docs/PLAN.md               the design and milestones
docs/reviews/              the evidence
scripts/check-tiers.sh     the tier boundary check
```
