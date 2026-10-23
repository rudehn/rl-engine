# Publishing the engine on crates.io as `atglyph`

## Intent

The engine is published on crates.io so a game depends on it by version, the way any Rust library is used.
The first user is Nate, whose games (Foundry, Undermere) will move to their own repositories and become ordinary dependents of the published crates.
The second is a stranger who finds it on crates.io and wants to know, within ten minutes, whether it fits their game.

That stranger copies the starter template and runs it.
They do not read code first to judge the engine.
Success is that the game they just generated shows a dungeon, a player, enemies, basic items and combat, all working, and that the code makes it obvious where each one lives.

## Goals

- Every engine crate is published on crates.io under the `atglyph` name, from the release workflow, at each `v*` tag.
- A game depends on `atglyph`, plus a `bevy` line at exactly the version `atglyph` re-exports, and nothing else of the engine's.
- `cargo generate` on the starter template produces a game that depends on `atglyph` by crates.io version and plays the five things above on its first run.
- The Bevy-free crates are usable on their own by a game that is not built on Bevy.
- Accidental breaking changes are caught before they are published.
- After the first release, the crates are listed where people looking for a roguelike engine look (done in the release project).

## Where this project ends

This project ends when pushing a `v*` tag would publish every crate cleanly: the rename is done, the template has its items, Bevy and `rand` are re-exported, the dry-run package of every crate passes in CI, and the release workflow runs the semver check and publishes (Nate's call).

The `0.4.0` release is the next project: the documentation pass over everything since `v0.3.0` (824 commits on 2026-10-22, with `scripts/check-systems.py` already failing), the version bump, the tag, and the listings and announcements.

These steps need Nate and are not done by the builder: creating the crates.io account and its trusted publisher or token, renaming the GitHub repository, pushing the tag, and posting the announcements.
The builder writes down exactly what each one involves, so Nate can do them when the release comes.

## Non-goals

- Reaching 1.0. The engine stays on `0.x` while Bevy does.
- Versioning crates independently of each other.
- Moving Foundry and Undermere out of this repository. That is planned separately; this project must not make it harder.
- A full inventory, shops or item identification in the template.

## Anti-goals

- A name that has to change later. Every crate name and version on crates.io is permanent.
- Placeholder crates published to hold names. crates.io's usage policy forbids reserving names with empty crates, so the names are taken by the first real release.
- A template whose first run needs anything beyond `cargo generate` and `cargo run`.
- A game that ends up with two copies of Bevy.

## Decisions

### The name

The engine is `atglyph`: the `@` the player is drawn as, which a roguelike player recognizes.
On 2026-10-22 the bare name and every sub-crate name below were free on crates.io, and there was no GitHub user, organization or repository by that name, and no company or product using it.
`rl-core` was already taken on crates.io (a token-bucket rate limiter), and "rl" reads as reinforcement learning to people searching crates.io, which is why the old prefix was not kept.
`atsign` was rejected because the Atsign Foundation, an unrelated networking company, uses the name.

| Today | Published as | Presented as |
|---|---|---|
| `rl-engine` | `atglyph` | the crate a game depends on |
| `rl-core` | `atglyph-core` | usable on its own |
| `rl-grid` | `atglyph-grid` | usable on its own |
| `rl-rules` | `atglyph-rules` | usable on its own |
| `rl-mapgen` | `atglyph-mapgen` | usable on its own |
| `rl-bevy` | `atglyph-bevy` | internal, reached through `atglyph` |
| `rl-render` | `atglyph-render` | internal |
| `rl-ui` | `atglyph-ui` | internal |
| `rl-save` | `atglyph-save` | internal |
| `rl-tty` | `atglyph-tty` | internal |

The GitHub repository is renamed `rudehn/atglyph`.
GitHub redirects the repository's old URLs, but not the old GitHub Pages address of the guide, so every link to the guide moves to the new address.
The rename is Nate's to do at the release, so links that name the repository or the guide's address change only when it is renamed, and until then keep working.

The rename is complete, not cosmetic: crate directories, package names, Rust paths (`rl_engine::` becomes `atglyph::`), the `[package.metadata.rl-engine]` key and the scripts that read it, the guide, the docs, the template and the examples all use the new name.
On 2026-10-22 the old names appeared about 8,300 times across about 390 files.

Three limits on the rename (Claude's calls):

- **History keeps its names.** Dated plans and specs in `docs/superpowers`, the evidence in `docs/reviews`, `.planning/`, `docs/PLAN.md`'s progress log and the CHANGELOG's entries before `0.4.0` describe what `rl-engine` was, and are not rewritten. A check for leftover old names, if one is added, excludes them.
- **The seed domain prefix becomes `atglyph.` (Nate's call), in a commit of its own after the rest of the rename.** `ENGINE_PREFIX` in `rl-core/src/seed.rs` is hashed into every engine stream, so changing it moves every seeded map: every fingerprint is re-baselined and every seed-pinned test re-picked, once, and an old recording replays differently, which the `0.4.0` CHANGELOG entry says. Landing the rest of the rename first, with the fingerprints unchanged, is what proves the mechanical rename changed no play (Claude's call).
- **The rename is a script that can be re-run.** Other agents keep merging to `main` while it lands, so the branch is rebased and the script run again rather than conflicts resolved by hand, and a branch in flight migrates by running the same script.

### Paths through the facade

`atglyph` re-exports each sub-crate under its short name, `atglyph::grid` rather than `atglyph::atglyph_grid` (Claude's call).
Two short names would clash and take another plain word: `atglyph-core` is `atglyph::primitives`, since `core` is Rust's own crate, and `atglyph-bevy` is `atglyph::ecs`, since `atglyph::bevy` is Bevy itself (Nate's call).
Games reach sub-crates through the facade about 760 times today as `rl_engine::rl_rules::...`, and these paths are public API every game and the `0.4.0` migration note carry, so they are settled once here.
Short names are also what makes a later merge of two sub-crates invisible to games.

### The crate split

All ten crates are kept.
Merging one after it is published costs one dead listing that stays on crates.io and one farewell release pointing to `atglyph`, which is small because games reach every crate through `atglyph` and its paths do not move.

The four Bevy-free crates are presented as usable on their own, because map generation, field of view, pathfinding and seeded dice are what people outside the engine are most likely to adopt, and the tier check in CI already keeps them free of Bevy and building for wasm.
Each gets a crate-level front page with a short example that uses no Bevy, and a README saying it is part of `atglyph`.

The Bevy crates are internal: their seams follow how the engine is organized, not something a stranger would use alone, and keeping them out of the spotlight leaves them free to change shape.
Each one's README says to depend on `atglyph` instead.

### Versions

All crates share the workspace version and are bumped and published together, as Bevy's own crates are.
The tag stays `v` plus the workspace version, and the CHANGELOG stays one file.
On `0.x` a breaking change bumps the minor version.

The first release under the new name is `0.4.0`.
Its CHANGELOG entry tells a game on `rl-engine 0.3` exactly what to change: the dependency line, the `use` paths, and dropping its own `bevy` line.

`cargo-semver-checks` runs against the last published version of every published crate, so a patch release that breaks the public API is refused before anything is published.
It runs in the release workflow, ahead of publishing, and by hand before tagging, not on every pull request: between releases `main` keeps the last release's version while it gathers the next one's breaking changes, so on a pull request it would fail every breaking change the next minor release is meant to carry (found while planning).
A crate with no published version yet, as every crate has at `0.4.0`, is skipped with a notice.

Each Bevy upgrade is a breaking release of `atglyph`.

### Bevy is re-exported, and a game still names it

`atglyph` re-exports Bevy (`pub use bevy;`), so a game can see which Bevy the engine is built on and reach it as `atglyph::bevy`.
A game must still list `bevy` in its own Cargo.toml, at the same version (found while planning, 2026-10-22, and replacing the design's first answer that a game would not list it).
Bevy's derive macros (`#[derive(Component)]`, `Resource`, `SystemParam` and the rest) find Bevy by reading the game's own Cargo.toml for a `bevy` dependency (`BevyManifest::maybe_get_path` in `bevy_macro_utils` 0.19), and with none they emit paths to `bevy_ecs` that do not resolve.

So the risk the re-export was meant to remove is held another way.
A game that names a different Bevy version than the engine ends up with two copies of Bevy, whose `App`, `Plugin` and `Component` are different types, and the errors that follow ("expected `App`, found `App`") are baffling to someone new.
The template pins its `bevy` line to the engine's Bevy version, CI fails when the two differ, the `bevy` line's comment says it must match, and every CHANGELOG entry for a Bevy upgrade says to change both lines.

`rand` is re-exported too, and since it has no derive macros a game can reach it through `atglyph` alone: the template and the games use `atglyph::rand` and list no `rand` of their own, because the engine's public API takes `&mut impl Rng` and hands out `rand` types, so a game on another `rand` hits the same two-copies error (Claude's call).
A `rand` upgrade is therefore a breaking release of `atglyph`.
`serde` is not re-exported: `serde` 1 has no competing major version to split on, and re-exporting it would put `#[serde(crate = "atglyph::serde")]` on every content type in the guide and the games.

### Publishing

The internal dependencies in `[workspace.dependencies]` carry `version = "<workspace version>"` beside `path`, which crates.io requires.
The release workflow publishes with `cargo publish --workspace` after the existing checks pass, so crates go up in dependency order.
Publishing uses crates.io trusted publishing from GitHub Actions rather than a stored API token where crates.io allows it.

CI runs a dry-run package of every published crate on every pull request, so a crate that would not publish (a missing file, a path outside the crate, an oversized package) fails long before a tag.

The example games and the template stay unpublished.

### The first ten minutes

The starter template keeps what it has: a floor of rooms, a player, goblins that hunt by sight, bump-to-attack combat with armor, light and stealth, panels, a help screen and tests.
It gains basic items: a potion to drink and a weapon or armor to wear, both found on the floor and picked up, using the engine's item and equipment plugins.

The generated game depends on `atglyph` by crates.io version rather than by git tag.
The `cargo generate` command is the first thing on `atglyph`'s crates.io page, its docs.rs front page and the repository README.
The template's "Where to go next" links point at the published guide, not the guide's source folder.

### Documentation

docs.rs builds every published crate's API docs.
`atglyph`'s crate docs open with the template command and a link to the guide; each other crate's opens with what it is and whether to use it on its own or through `atglyph`.
The guide stays published at each tag as it is today.
The documentation pass that already comes before every release covers the published pages as well.

### Being found

After the first release the engine is listed on Bevy Assets, arewegameyet.rs and RogueBasin's library list, and announced on r/roguelikedev's Sharing Saturday, r/rust and This Week in Rust.
The playable tutorial demos the guide already builds for the browser are linked from the README and the crates.io page.

## Left to the builder

- The order of the work, and whether the rename, the publishing setup and the template's items land as one branch or several.
- How the template's items are wired, within the template's existing style of one `src/main.rs` read top to bottom.
- Which docs.rs build settings each crate needs to build there.
- How the release workflow handles crates.io's rate limit on publishing new crates, which the first release of ten new names may hit.
- Whether trusted publishing can be used for the very first publish or only from the second release on.
- The exact wording of the front pages and READMEs.
