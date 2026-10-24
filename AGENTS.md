# Agent guide

`CLAUDE.md` is a symlink to this file. Edit this one.

## What this is

`atglyph` is a multi-crate roguelike engine. Read `README.md` for the crate tiers and `docs/PLAN.md` for every design decision and the milestone plan.
The plan was written against four code reviews in `docs/reviews/`; when a decision looks odd, the reason is cited there.

## Rules that the build enforces

- Every workspace member declares `tier` under `[package.metadata.atglyph]` in its `Cargo.toml`. Tier 0 and 1 crates must not depend on Bevy, and no crate depends on a higher tier. `scripts/check-tiers.sh` reads the tiers from `cargo metadata` and checks both; CI runs it.
- `#![deny(missing_docs)]` on every crate. Doc-tests compile and run; never fence an example as `ignore`.
- **The documentation is paid once, at a release.** Not in every commit and not at every branch's end: re-reading and re-blessing pages per commit was measured at 195 of 197 page edits changing nothing but the fingerprint, and a pass per branch still cost every feature its own hours of reading, so the docs are brought up to the code once, before a version is tagged. A commit runs the fast gate, `scripts/gate.sh`: `cargo fmt --all --check`, clippy and the fast tier of the tests of what it touched (while iterating, `cargo test -p <crate> <filter>`; a whole suite runs with `cargo nextest run` plus `cargo test --doc`, which nextest skips, and tests are never timed while other builds compile; the everyday run plays the first 8 seeds of a generation test and `ATGLYPH_FULL_SEEDS=1` plays the full range, as CI does; "Writing fast tests" below holds the suite to its budget), and writes its `CHANGELOG.md` line while the migration is fresh; that line is the record the release's documentation pass works from, so it is never skipped. Finishing a branch runs the code half of CI, everything but the documentation checks and with the slow tier of the tests, which is `scripts/gate.sh --all`. CI on `main` and on pull requests leaves the documentation checks off, so `main` stays green while the pages lag the code; the release workflow, on a `v*` tag, runs CI with them on (`docs: true`) before it publishes the release page, and the guide is published at the same tag. So a release is: the documentation pass below over everything since the last tag, the version bumped, then the tag pushed. Five files, and which of them depends on what changed since the last release:
  - `docs/guide/src/systems/<system>.md`, always. It is the page a game author reads to use the system, and it carries a `documents:` manifest naming the plugins and the source files it covers. `scripts/check-systems.py` fails when one of those files has changed since the page was confirmed, and prints the `git diff` of what moved; read that diff against the page, fix what it made untrue, then `scripts/check-systems.py --bless <system>`. It fails the same way when a plugin in `crates/` has no page naming it at all; write the page.
  - `docs/OVERVIEW.md`, always. It is the inventory of what the engine has, and the plugin table in its `atglyph-bevy` section and the presenter table in its `atglyph-ui` section are part of it, not a summary of it.
  - `CHANGELOG.md`, under `Unreleased`, whenever a game on the previous release would have to change a line or would want to. That is the only place a game author reads what moved.
  - `README.md`'s feature list, when the slice is something a reader shopping for an engine would look for. Not for an addition to a system already listed.
  - `docs/design/<subsystem>.md`, when the slice is a new subsystem rather than a change to one. The reasoning and the rejected alternatives go there; the inventory stays a list.

  `scripts/check-overview.sh` enforces what can be enforced exactly: every `impl Plugin for X` is named in the overview, every `docs/design/*.md` is listed in the layout section below, and every game in `examples/` is both described in the overview and named in the guide. The release's CI runs it. It cannot check that a sentence is still true, so that stays a review matter.

  `docs/README.md` is the index GitHub renders when someone opens the `docs` folder, and a new design doc goes on it.
  Only `docs/guide` is published as a website, so a link from a guide chapter to anything outside it is an absolute `https://github.com/rudehn/rl-engine/blob/main/...` link, which resolves from the published book and from GitHub both; a relative one resolves only on GitHub.
- `cargo fmt --all --check` must pass; `rustfmt.toml` pins the width, so do not hand-wrap.
- `cargo clippy --workspace --all-targets -- -D warnings` must pass. Do not add crate-wide `#![allow(clippy::too_many_arguments)]`; a system with sixteen parameters is a system to split.
- Tier 0 and 1 crates build on `wasm32-unknown-unknown`, and so does `atglyph-save`, whose browser storage and unload bridge exist only there; `scripts/check-tiers.sh --wasm` checks all of them. No `std::time::Instant` in them.

## Building beside other agents

Several agents build and test on one machine at once, and these keep them from starving each other.
Branch work happens in a worktree made with `scripts/worktree.sh <name>`, never by switching the main checkout's branch: its `target/` starts as a copy-on-write clone of the main checkout's, so Bevy is not compiled again.
Every compile and every test process takes one of the machine's slots (`scripts/slot`), so builds queue rather than swap; a build that waits is not hung.
Bevy is linked as a shared library on the desktop, so run a game with `cargo run -p <game>`, which tells the loader where it is, not by the binary's path.
A gate builds the whole workspace, as `scripts/gate.sh` does; `-p <crate>` is for iterating, since a build narrowed to one crate may unify a dependency's features differently and compile a second copy of it.

## Writing fast tests

nextest pools every test, so on a quiet machine the suite's wall time is the processor time of all its tests over the cores, and a test that plays its seeds on every core still costs the suite all of that time.
So the suite is held to a budget of processor time, which a busy machine barely inflates, where it can triple the wall time.

The tests come in two tiers, and nextest knows which is which:

- **The fast tier** is unit tests, properties tested without an `App`, small `App` tests and the fingerprints. It is nextest's default profile, so `cargo nextest run` and the scoped `scripts/gate.sh` that every commit runs play it and nothing else.
- **The slow tier** is the full-game play-throughs, each game's backstops over the whole generator, and any test past two seconds of processor time. `.config/nextest.toml` names each of them by package and path in the default profile's `default-filter`. It runs once, at a branch's end and before it merges, in `scripts/gate.sh --all` (`--profile full`), and in CI (`--profile ci`); both profiles run both tiers.
- **The fingerprints stay in the fast tier at any cost.** They are the early warning that a change moved play, and Undermere's four scripted delves are its dearest test, about 60 s of processor time and 17 s alone.
- **A new test that builds a full game `App` and plays turns or floors goes in the slow tier unless it is cheap.** The check names any fast-tier test past two seconds as one to make cheaper or move there.
- **While working on an area**, run its slow tests by filter, with `--profile full` or `--ignore-default-filter` so the default filter does not drop them: `cargo nextest run --workspace --profile full -E 'package(undermere) & test(/traps::/)'`. `cargo test -p <crate> <filter>` runs every test the filter names, slow or not.

Every test asks one of four kinds of question, and each kind is answered its own way:

| Kind | The question | How it is tested |
|---|---|---|
| Code | Does a pass, a draw or a rule do what it says? | A unit or property test on made-up data, in the crate the code lives in. |
| Content presence | Can every entry of a file show up? | Validated statically: the loader refuses a file with every problem at once, naming the entry and why it could never appear, and what no loader can ask is a fast test that reads the content and grows no place. |
| Content spread | Does content come up as often as the design says? | It follows from the weights: a share worth guarding is computed from the file with no draw, as a static check, and otherwise it is not tested. |
| Generator promises | Does the whole chain keep its promises with real content? | One backstop per game in the slow tier, each place grown once a seed at `seeds(full)` and asked every promise, beside a property test of the pass that owns each promise on a made-up map. |

So generation is never played over many seeds to see whether content turns up or how often: the code is tested, the content is validated, and the two together say it turns up.

`scripts/slot` records each test's processor time beside the wall time nextest records, and `scripts/test-times.py` reads both after every gate:

- processor time over the budget fails the gate: both tiers of the workspace over 900 s, its fast tier over 490 s, Undermere's fast tier alone over 310 s;
- any one test over 120 s of processor time fails it, and one over 20 s, or 30 s of wall time, is named;
- wall time over its budget only warns, since a busy machine explains it: 170 s for both tiers, 110 s for the fast tier, 60 s for Undermere's;
- CI plays every seed on another machine, so it reports against numbers of its own and only warns.

Each run's totals by package are kept in `rl-test-times.tsv` in the main checkout's `.git` (`git rev-parse --git-common-dir`), one line a package a run with its branch and commit, so a branch's times outlive its worktree.

When the check speaks, the slowest tests it lists are the ones to fix, by the cheapest of these that keeps the test proving what it proved:

- **A property of generation is tested without an `App`.** Build the place (`Undermere::build`, an `atglyph-mapgen` chain) and read it; arriving through a headless game populates, lights and saves it as well. A rule that needs no world goes to `atglyph-rules` and is tested there.
- **A rule inside a system is a rule only an `App` can test.** So a system gathers, calls a function in tier 1 (`atglyph-rules`, or `atglyph-core` and `atglyph-grid` for arithmetic and grid search), and applies the result; the `App` test that is left proves the call, and the rule's cases are unit tests (`atglyph_rules::faction::stance` is the model).
- **A property over seeds plays `seeds(full)`.** It is the first `QUICK` seeds, eight, on every run, and all of `full` under `ATGLYPH_FULL_SEEDS=1`, which CI sets, so the rare seed is still caught. A property never plays a bare range.
- **A rate or a count of rare things plays `first_seeds(full, n)`**, with the reason beside it: such a test needs `n` seeds before its count says anything.
- **A test waiting for a rare thing stops at the first one.** Build floors until the thing turns up, and no further, rather than every floor of every seed (`a_blood_trail_turns_up_on_some_floors_but_never_the_throne_over_a_span_of_seeds`).
- **What does not change per seed is made once.** Content is read once and reseeded (`Undermere::reseeded`); a floor walked for every cell a check asks from is walked once and handed in (`far_choices_among`).
- **State is set up, not played to.** Stand someone where the test needs them, give them the thing, clear the monsters: the helpers in `atglyph::testing` and a game's own `testing` module do it in a frame; turns are played only when the turns are the property.
- **A fingerprint is never sped up by changing what it plays**; it is a tripwire, and a faster one that moves is a different run.
- **`#[ignore]` is for a by-hand span or a tuning aid**, its reason in the attribute, never for an everyday test that grew slow; the slow tier is where a slow everyday test goes.

A budget is raised only on purpose, saying why in the commit, as a fingerprint is re-baselined.

### Soaks

A soak (`examples/undermere/tests/soak.rs`) is a by-hand tuning aid of about ten minutes on a quiet machine, not a test.

- **Soak only what changed.** `examples/undermere/SOAK.md` records each soak's numbers beside the commit they were measured at. A baseline is that latest row; re-soak it only when `git diff --quiet <commit> HEAD -- crates examples/undermere/src examples/undermere/assets` says game code changed.
- **One soak at a time on the machine.** It already uses every core, so two at once each take twice as long; check `pgrep -f 'test soak'` first and wait.
- **Run it on the dev profile.** Dependencies are already optimized, and a release build costs a second full build and drops the overflow checks a soak should catch.
- **Record the result** as a row of `SOAK.md` in the commit that tuned against it.

## Rules that only review enforces

- **Own the loop or leave it out.** The previous engine shipped data structures and left the behaviour in the game, and the game copied the data structures instead of depending on it.
- **No theme words in engine crates.** No fantasy, sci-fi or pirate vocabulary in types, docs or constants. Content is an id in a registry.
- **No `#[non_exhaustive]` + `Custom { id }`** and no closed taxonomy enums for content. Registries and traits are the only extension mechanisms.
- **Randomness** comes from `rand`, through `RunSeed::derive(domain, index)`: a subsystem's stream is a `Stream` registered with `add_stream`, derived from the run's `Seed`, and a game's own draws come from `Seed::stream`. Functions take `&mut impl Rng`. Never construct a generator from a constant or from entropy inside engine code.
- **No `HashMap` or `HashSet` in gameplay or generation paths.** `BTreeMap`, `Vec`, or a `BitGrid`, with the reason stated.
- **A subsystem is a plugin, and a plugin is opt-in.** Whether a system runs is decided by whether its plugin was added, never by whether a resource happens to be there. A plugin declares what it cannot work without with `app.needs::<R>(plugin, hint)`, where the hint says how a game makes one, and everything missing is reported together, loudly, when play begins.
- **A panel is a view, a collector and a presenter.** Never one system that queries and draws. The view is a resource of plain data with no `Color`, no `Rect` and no string the game did not supply; the collector refills it in `ViewSet::Collect`; the presenter draws it in a `PresentSet` layer and takes its rectangle in its constructor. Arithmetic that is really about the rules goes to `atglyph-rules` where it is tested without an `App`. `atglyph-ui/src/lib.rs` is the order to write them in.
- **A widget takes a `ToneId`, never a `Color`.** Colours live in the `Palette`, indexed by an interned tone. A widget that took a colour is a widget every game forks.
- **What the engine cannot know is a `Facet`, not a field.** A wielded weapon, an AI state, a resist chip: the game pushes `Facet { key, text, tone }` onto a row in `ViewSet::Annotate`. Two games pushing the same key is the signal the field belongs in the view.
- **A game's rules answer the turn inside it.** Anything that reacts to what a turn caused goes in `TurnSet::React`, not in the drawing phase; drawing goes in its `PresentSet` layer. Never order a system after another crate's system function.
- **Costs and clocks are integers.** Hundredths of a step, the same unit everywhere.
- **Doc comments say why and why-not**, at the density of `atglyph-core/src/turn.rs`, not more.
- **A resource may still be optional data.** The distinction is whether the subsystem is optional or the game's setup is wrong. `Lighting` absent means a lit world and no cost; no statuses in `Registries` means no badges on a health bar. But a `GearPanel` in a game with no `Registries` at all is a mistake, so it asserts. Read an optional one as `Option<Res<_>>` and say in the docs what its absence means.
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
scripts/check-systems-style.sh  the mechanical half of a system page's review
scripts/gate.sh                 a commit's gate, scoped to what the branch touched; --all for a branch's end
scripts/worktree.sh             a worktree for a new branch, its build cloned warm from the main checkout's
scripts/rustc-wrapper           every compile, through sccache, inside a machine-wide slot
scripts/slot                    the slots: one compile or test process per core, across every agent; records each test's processor time
scripts/test-times.py           the slowest tests of the last run, and the suite held to its budget
```

The subsystems with no design doc yet are the turn loop with its cues and the wait on them, combat and `Loadout`, equipment and what a worn thing lends, saving and the end of a run, the title and its hold on a run's start, registries and content loading, the controls, modals and cursors, trade between two actors, the party rail, the standard keys and the play screen's layout, and the testing harness a game's tests stand on.
Each is documented in its crate's module docs and in the overview; what is missing is the page that says why it is shaped that way.
Writing one is a welcome slice, not a prerequisite for touching the code.

## Building UI

`docs/design/ui.md` is the design and `crates/atglyph-ui/src/lib.rs` is the how-to; read the crate docs before adding a panel.
The short version: `UiPlugin` is the base, each panel is its own plugin taking a `Rect`, and a presenter adds its view plugin behind it.
`docs/guide/src/09-where-to-go-next.md` is the same thing aimed at a game author, and `examples/tutorial/src/bin/step09_panels.rs` is the worked example the chapter quotes.
Undermere is the full set: vitals, gear, nearby and inspect with a game facet for a townsperson's role, log, the sheet, a party, trade and records, and its screens on the modal stack; Foundry's gear rows carry a game facet for a weapon's heat.

## Writing a system page

`docs/guide/src/systems/<system>.md` is what a game author reads to use a system, and `docs/design/<system>.md` is why it is shaped that way.
The reference is published; the design notes are not.

Six parts, fixed, in this order: one paragraph saying what the system is, then `Turning it on`, `The model`, `Using it`, `The line`, `Where it lives`.
`The line` says what the engine decides and what the game decides, and it is the section a reader arrives for, so it is never the section cut to fit the budget.
`The model` names the public surface and is written from the type definitions and the system bodies; a design note is read for `The line` only, because a design note records what was intended and the code records what is.
`Using it` opens with one sentence naming what the snippet is an instance of, then the snippet; a second anchor only when turning the system on takes a step the first does not show.
`Where it lives` says what the crate split buys, such as what can be tested without an `App`, rather than which file holds what, which the manifest at the top of the page already answers.
No `Cost`, no `Risks`, no `Phases`; those stay in the design notes.

Every code block is an `expand-guide.py` include of a real anchor in `examples/`, fenced `rust,no_run` as the rest of the guide is.
A page carries no invented code, and the only comments it carries are the manifest and one `include:` marker per snippet.
The manifest's `files:` lists every source file the page makes a claim about, not only the crate the plugin lives in, because the fingerprint guards exactly what it lists and a page describing a tier 1 type from a tier 2 facade otherwise goes stale without anything noticing.
`registries.md` is the page that found this: a resource of eight fields in `atglyph-bevy` over the `Registry` and `Names` in `atglyph-rules` that `The model` is really about.
The same rule read the other way keeps `crates/atglyph-bevy/src/plugin.rs` off most manifests: naming the set a system runs in is a claim about the `add_systems` call in the page's own plugin file, which it already guards, and not about the file that declares the set, so a page lists the hub only where it says something `plugin.rs` alone can answer, such as what `CorePlugin` registers or where a set sits in its chain.
The budget is eighty lines of prose, counted by `scripts/check-systems-style.sh` over the page's own sentences: the manifest, the `include:` markers and the fenced code are all excluded, so guarding one more file or quoting a longer anchor never costs the page a sentence.
It was a cap on file length until 2026-09-22, when the two rules were measured against each other and the pages pressed hardest against it turned out to have the least writing in them, `noise.md` at one line under the limit with 48 lines of prose against `fields.md` comfortable at 94 with 50.
A page may therefore run to any length its manifest and its anchors need; reinstating a length cap would bring the perverse case back, so do not.
Run `scripts/check-systems-style.sh <page>` after writing a page, and `python3 scripts/check-systems.py --bless <system>` after confirming it against the code that moved, both in the documentation pass that comes before a release.
