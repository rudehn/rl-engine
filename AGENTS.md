# Agent guide

`CLAUDE.md` is a symlink to this file. Edit this one.

## What this is

`rl-engine` is a multi-crate roguelike engine. Read `README.md` for the crate tiers and `docs/PLAN.md` for every design decision and the milestone plan.
The plan was written against four code reviews in `docs/reviews/`; when a decision looks odd, the reason is cited there.

## Rules that the build enforces

- Every workspace member declares `tier` under `[package.metadata.rl-engine]` in its `Cargo.toml`. Tier 0 and 1 crates must not depend on Bevy, and no crate depends on a higher tier. `scripts/check-tiers.sh` reads the tiers from `cargo metadata` and checks both; CI runs it.
- `#![deny(missing_docs)]` on every crate. Doc-tests compile and run; never fence an example as `ignore`.
- `docs/OVERVIEW.md` is the inventory of what the engine has; a slice that adds or removes a system updates it in the same commit.
- `cargo fmt --all --check` must pass; `rustfmt.toml` pins the width, so do not hand-wrap.
- `cargo clippy --workspace --all-targets -- -D warnings` must pass. Do not add crate-wide `#![allow(clippy::too_many_arguments)]`; a system with sixteen parameters is a system to split.
- Tier 0 and 1 crates build on `wasm32-unknown-unknown`; `scripts/check-tiers.sh --wasm` checks it. No `std::time::Instant` in them.

## Rules that only review enforces

- **Own the loop or leave it out.** The previous engine shipped data structures and left the behaviour in the game, and the game copied the data structures instead of depending on it.
- **No theme words in engine crates.** No fantasy, sci-fi or pirate vocabulary in types, docs or constants. Content is an id in a registry.
- **No `#[non_exhaustive]` + `Custom { id }`** and no closed taxonomy enums for content. Registries and traits are the only extension mechanisms.
- **Randomness** comes from `rand`, through `RunSeed::derive(domain, index)`: a subsystem's stream is a `Stream` registered with `add_stream`, derived from the run's `Seed`, and a game's own draws come from `Seed::stream`. Functions take `&mut impl Rng`. Never construct a generator from a constant or from entropy inside engine code.
- **No `HashMap` or `HashSet` in gameplay or generation paths.** `BTreeMap`, `Vec`, or a `BitGrid`, with the reason stated.
- **A subsystem is a plugin, and a plugin is opt-in.** Whether a system runs is decided by whether its plugin was added, never by whether a resource happens to be there. A plugin declares what it cannot work without with `app.needs::<R>(plugin, hint)`, where the hint says how a game makes one, and everything missing is reported together, loudly, when play begins.
- **A panel is a view, a collector and a presenter.** Never one system that queries and draws. The view is a resource of plain data with no `Color`, no `Rect` and no string the game did not supply; the collector refills it in `ViewSet::Collect`; the presenter draws it in a `PresentSet` layer and takes its rectangle in its constructor. Arithmetic that is really about the rules goes to `rl-rules` where it is tested without an `App`. `rl-ui/src/lib.rs` is the order to write them in.
- **A widget takes a `ToneId`, never a `Color`.** Colours live in the `Palette`, indexed by an interned tone. A widget that took a colour is a widget every game forks.
- **What the engine cannot know is a `Facet`, not a field.** A wielded weapon, an AI state, a resist chip: the game pushes `Facet { key, text, tone }` onto a row in `ViewSet::Annotate`. Two games pushing the same key is the signal the field belongs in the view.
- **A game's rules answer the turn inside it.** Anything that reacts to what a turn caused goes in `TurnSet::React`, not in the drawing phase; drawing goes in its `PresentSet` layer. Never order a system after another crate's system function.
- **Costs and clocks are integers.** Hundredths of a step, the same unit everywhere.
- **Doc comments say why and why-not**, at the density of `rl-core/src/turn.rs`, not more.
- **A resource may still be optional data.** The distinction is whether the subsystem is optional or the game's setup is wrong. `Lighting` absent means a lit world and no cost; `StatusRules` absent means no badges on a health bar. But a `GearPanel` with no slot registry is a mistake, so it asserts. Read an optional one as `Option<Res<_>>` and say in the docs what its absence means.
- **Tests**: property-over-seed-range where a property exists, fingerprint tripwires labelled as such where none does, and a name that reads as a sentence describing the property.
- **Every RON schema** carries a top-of-file comment listing the full option space.
- **No `TODO` comments in source.** Outstanding work is tracked in `docs/PLAN.md` or an issue.
- Prose style: plain dash, never an em dash; American spelling in identifiers.

## Layout

```
crates/<name>/src/lib.rs   module-level //! docs name every public item
crates/<name>/benches/     criterion, on realistic maps, only for hot paths
docs/PLAN.md               the design and milestones
docs/design/               how one subsystem works, and why: lighting, ui, abilities, stealth
docs/guide/                the mdBook; every chapter quotes examples/tutorial
docs/reviews/              the evidence
scripts/check-tiers.sh     the tier boundary check
scripts/check-guide.sh     the guide's includes, images and contents
```

## Building UI

`docs/design/ui.md` is the design and `crates/rl-ui/src/lib.rs` is the how-to; read the crate docs before adding a panel.
The short version: `UiPlugin` is the base, each panel is its own plugin taking a `Rect`, and a presenter adds its view plugin behind it.
`docs/guide/src/10-panels.md` is the same thing aimed at a game author, and `examples/tutorial/src/bin/step10_panels.rs` is the worked example the chapter quotes.
Corsair is the full set: vitals, gear, nearby with a game facet for what an enemy wields, log, inspect, and two screens on the modal stack.
