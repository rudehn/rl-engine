# Agent guide

`CLAUDE.md` is a symlink to this file. Edit this one.

## What this is

`atglyph` is a multi-crate roguelike engine. Read `README.md` for the crate tiers and `docs/PLAN.md` for every design decision and the milestone plan.
When a decision looks odd, the reason is cited in `docs/reviews/`.

## Rules that the build enforces

- Every workspace member declares `tier` under `[package.metadata.atglyph]` in its `Cargo.toml`. Tier 0 and 1 crates must not depend on Bevy, and no crate depends on a higher tier; `scripts/check-tiers.sh` checks both.
- `#![deny(missing_docs)]` on every crate. Doc-tests compile and run; never fence an example as `ignore`.
- `cargo fmt --all --check` must pass; `rustfmt.toml` pins the width, so do not hand-wrap.
- `cargo clippy --workspace --all-targets -- -D warnings` must pass. Do not add crate-wide `#![allow(clippy::too_many_arguments)]`; a system with sixteen parameters is a system to split.
- Tier 0 and 1 crates build on `wasm32-unknown-unknown`, and so does `atglyph-save`; `scripts/check-tiers.sh --wasm` checks all of them, in CI. No `std::time::Instant` in them.
- A commit runs `scripts/gate.sh`: fmt, the name and prelude checks, clippy, `cargo test --workspace` and the tier check. CI runs the same, plus every seed, the wasm build and the docs.
- A commit message is one subject line and, if it needs one, a short body.
- A commit that changes what a game sees adds one line to `CHANGELOG.md` under `## [Unreleased]`, in its [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) group (`Added`, `Changed`, `Deprecated`, `Removed`, `Fixed`, `Security`). One line per change; a breaking one starts `**Breaking:**` and says what a game changes.

## The documentation pass, at a release

The documentation is brought up to the code once, before a version is tagged, not in every commit.
CI on `main` leaves the documentation checks off; the release workflow, on a `v*` tag, runs them before it publishes.
A release is the pass below over everything since the last tag, then the version bump, then the tag (`docs/RELEASING.md`).

- `docs/guide/src/systems/<system>.md`, always. Its `documents:` manifest names the plugins and source files it covers; `scripts/check-systems.py` fails when one has changed since the page was confirmed and prints the diff. Fix what the diff made untrue, then `scripts/check-systems.py --bless <system>`. A plugin with no page fails it too; write the page.
- `docs/OVERVIEW.md`, always. It is the inventory, and its plugin and presenter tables are part of it.
- `CHANGELOG.md`: read `Unreleased` against what landed, then rename it to the version.
- `README.md`'s feature list, when the release adds something a reader shopping for an engine would look for.
- `docs/design/<subsystem>.md`, for a new subsystem: the reasoning and the rejected alternatives.

`scripts/check-overview.sh` checks that every `impl Plugin for X` is named in the overview, every `docs/design/*.md` is listed in the layout section below, and every game in `examples/` is in the overview and the guide.
`docs/README.md` is the index GitHub shows for the `docs` folder; a new design doc goes on it.
Only `docs/guide` is published, so a guide chapter links outside it with an absolute `https://github.com/rudehn/rl-engine/blob/main/...` link.

## Building beside other agents

Several agents build and test on one machine at once.
Branch work happens in a worktree made with `scripts/worktree.sh <name>`, never by switching the main checkout's branch; its `target/` starts as a copy-on-write clone of the main checkout's, so Bevy is not compiled again.
Every compile takes one of the machine's slots (`scripts/slot`), so builds queue rather than swap; a build that waits is not hung.
Bevy is linked as a shared library on the desktop, so run a game with `cargo run -p <game>`, not by the binary's path.
A gate builds the whole workspace; `-p <crate>` is for iterating, since a narrowed build may unify features differently and compile a second copy of a dependency.
Before merging, read what landed on `main` since the branch left it, rebase, and run the gate again if anything did.

## Tests

While iterating, `cargo test -p <crate> <filter>`; before a commit, `scripts/gate.sh`.
A generation test plays the first 8 seeds of its range, and `ATGLYPH_FULL_SEEDS=1`, which CI sets, plays all of them.
Keep the suite fast: a test past two seconds is made cheaper.

Every test asks one of four kinds of question:

| Kind | The question | How it is tested |
|---|---|---|
| Code | Does a pass, a draw or a rule do what it says? | A unit or property test on made-up data, in the crate the code lives in. |
| Content presence | Can every entry of a file show up? | The loader refuses a file with every problem at once, naming the entry and why it could never appear. |
| Content spread | Does content come up as often as the design says? | A share worth guarding is computed from the weights with no draw; otherwise it is not tested. |
| Generator promises | Does the whole chain keep its promises with real content? | A property test of the pass that owns each promise, on a made-up map. |

So generation is never played over many seeds to see whether content turns up or how often.

The cheapest ways to keep a test fast while it still proves what it proved:

- A property of generation is tested without an `App`: build the place with an `atglyph-mapgen` chain and read it.
- A rule inside a system lives in tier 1 (`atglyph-rules`, or `atglyph-core` and `atglyph-grid`), where its cases are unit tests; the `App` test only proves the call. `atglyph_rules::faction::stance` is the model.
- A property over seeds plays `seeds(full)`; a rate or count of rare things plays `first_seeds(full, n)`, with the reason beside it. Never a bare range.
- A test waiting for a rare thing stops at the first one.
- What does not change per seed is made once.
- State is set up, not played to: the helpers in `atglyph::testing` stand someone where the test needs them in a frame.
- A fingerprint is a tripwire; it is re-baselined on purpose, never sped up by changing what it plays.
- `#[ignore]` is for a by-hand span or a tuning aid, its reason in the attribute.

## Rules that only review enforces

- **Own the loop or leave it out.** The previous engine shipped data structures and left the behaviour in the game, and the game copied the data structures instead of depending on it.
- **No theme words in engine crates.** No fantasy, sci-fi or pirate vocabulary in types, docs or constants. Content is an id in a registry.
- **No `#[non_exhaustive]` + `Custom { id }`** and no closed taxonomy enums for content. Registries and traits are the only extension mechanisms.
- **Randomness** comes from `rand`, through `RunSeed::derive(domain, index)`: a subsystem's stream is a `Stream` registered with `add_stream`, derived from the run's `Seed`, and a game's own draws come from `Seed::stream`. Functions take `&mut impl Rng`. Never construct a generator from a constant or from entropy inside engine code.
- **No `HashMap` or `HashSet` in gameplay or generation paths.** `BTreeMap`, `Vec`, or a `BitGrid`, with the reason stated.
- **A subsystem is a plugin, and a plugin is opt-in.** Whether a system runs is decided by whether its plugin was added, never by whether a resource happens to be there. A plugin declares what it cannot work without with `app.needs::<R>(plugin, hint)`, and everything missing is reported together when play begins.
- **A panel is a view, a collector and a presenter.** Never one system that queries and draws. The view is a resource of plain data with no `Color`, no `Rect` and no string the game did not supply; the collector refills it in `ViewSet::Collect`; the presenter draws it in a `PresentSet` layer and takes its rectangle in its constructor. `atglyph-ui/src/lib.rs` is the order to write them in.
- **A widget takes a `ToneId`, never a `Color`.** Colours live in the `Palette`, indexed by an interned tone.
- **What the engine cannot know is a `Facet`, not a field.** The game pushes `Facet { key, text, tone }` onto a row in `ViewSet::Annotate`. Two games pushing the same key is the signal the field belongs in the view.
- **A game's rules answer the turn inside it.** Anything that reacts to what a turn caused goes in `TurnSet::React`; drawing goes in its `PresentSet` layer. Never order a system after another crate's system function.
- **Costs and clocks are integers.** Hundredths of a step, the same unit everywhere.
- **Doc comments say why and why-not**, at the density of `atglyph-core/src/turn.rs`, not more.
- **A resource may still be optional data.** Whether it is depends on whether the subsystem is optional or the game's setup is wrong: `Lighting` absent means a lit world, but a `GearPanel` in a game with no `Registries` asserts. Read an optional one as `Option<Res<_>>` and say in the docs what its absence means.
- **A game uses only the public API.** A game names its own systems and the engine's system sets, components, resources, messages and plugins, never an engine system function or a private path. [Undermere](https://github.com/rudehn/undermere) and [Foundry](https://github.com/rudehn/foundry) live in repositories of their own, pinned by tag, and are the proof.
- **Tests**: property-over-seed-range where a property exists, fingerprint tripwires labelled as such where none does, and a name that reads as a sentence describing the property.
- **Every RON schema** carries a top-of-file comment listing the full option space.
- **No `TODO` comments in source.** Outstanding work is tracked in `docs/TODO.md`, the plan's progress log, or an issue.
- Prose style: plain dash, never an em dash; American spelling in identifiers.

## Layout

```
crates/<name>/src/lib.rs        module-level //! docs name every public item
crates/<name>/benches/          criterion, on realistic maps, only for hot paths
docs/PLAN.md                    the design and milestones
docs/design/                    how one subsystem works, and why: abilities, accuracy, effects, fields,
                                items, layout, lighting, loot, minds, names, noise, places, prefabs, props,
                                records, remains, settings, stealth, tilesets, tty, ui, watching, work
docs/guide/                     the mdBook; every chapter quotes examples/tutorial
docs/reviews/                   the evidence
scripts/check-tiers.sh          the tier boundary check
scripts/check-guide.sh          the guide's includes, images and contents
scripts/check-overview.sh       the inventory names every plugin, design doc and example
scripts/check-systems.py        the reference keeps up with the code
scripts/gate.sh                 a commit's gate
scripts/worktree.sh             a worktree for a new branch, its build cloned warm from the main checkout's
scripts/rustc-wrapper           every compile, through sccache, inside a machine-wide slot
scripts/slot                    the slots: one compile per core, across every agent
```

The subsystems with no design doc yet are the turn loop with its cues, combat and `Loadout`, equipment, saving and the end of a run, the title, registries and content loading, the controls, modals and cursors, trade, the party rail, the standard keys and the play screen's layout, and the testing harness.
Each is documented in its crate's module docs and in the overview; writing its design doc is a welcome slice, not a prerequisite for touching the code.

## Building UI

`docs/design/ui.md` is the design and `crates/atglyph-ui/src/lib.rs` is the how-to; read the crate docs before adding a panel.
`UiPlugin` is the base, each panel is its own plugin taking a `Rect`, and a presenter adds its view plugin behind it.
`docs/guide/src/09-where-to-go-next.md` is the same aimed at a game author, and `examples/tutorial/src/bin/step09_panels.rs` is the worked example it quotes.
[Undermere](https://github.com/rudehn/undermere) is the full set, its screens on the modal stack; [Foundry](https://github.com/rudehn/foundry)'s gear rows carry a game facet for a weapon's heat.

## Writing a system page

`docs/guide/src/systems/<system>.md` is what a game author reads to use a system, and `docs/design/<system>.md` is why it is shaped that way; only the first is published.

Six parts, in this order: one paragraph saying what the system is, then `Turning it on`, `The model`, `Using it`, `The line`, `Where it lives`.
`The line` says what the engine decides and what the game decides; it is the section a reader arrives for, so it is never the one cut.
`The model` names the public surface and is written from the type definitions and the system bodies, since the code records what is and a design note what was intended.
`Using it` opens with one sentence naming what the snippet is an instance of, then the snippet.
`Where it lives` says what the crate split buys, such as what can be tested without an `App`.
No `Cost`, no `Risks`, no `Phases`; those stay in the design notes.

Every code block is an `expand-guide.py` include of a real anchor in `examples/`, fenced `rust,no_run`; a page carries no invented code.
The manifest's `files:` lists every source file the page makes a claim about, not only the plugin's crate, because the fingerprint guards exactly what it lists.
`crates/atglyph-bevy/src/plugin.rs` goes on a manifest only where the page says something only it can answer, such as what `CorePlugin` registers.
Keep a page as short as what it says allows.
