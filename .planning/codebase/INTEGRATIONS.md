# External Integrations

**Analysis Date:** 2026-09-17

This is a game engine and its example games, not a networked application.
There are no third-party web APIs, payment providers or SaaS backends.
The integrations that exist are: the Bevy engine itself, browser storage on wasm, GitHub-hosted CI/CD, and a still-pending crates.io publish.

## Game Engine Integration

**Bevy 0.19:**
- The whole tier 2/3 layer (`rl-bevy`, `rl-render`, `rl-overworld`, `rl-save`, `rl-ui`, `rl-engine`) is built as a set of Bevy plugins on top of `App`.
- Tier 0 and tier 1 crates (`rl-core`, `rl-grid`, `rl-mapgen`, `rl-world`, `rl-rules`) have zero Bevy dependency by rule; `scripts/check-tiers.sh` fails the build if one gains one.
- A game assembles the engine from `RoguelikePlugins::new(title, width, height)` plus opt-in plugins (`CombatPlugin`, `MindsPlugin`, etc.); nothing runs unless its plugin was added (`README.md`, `CLAUDE.md`).
- Missing resources a plugin needs are declared with `app.needs::<R>(plugin, hint)` and reported together at startup rather than causing a silent no-op or a panic deep in a system (`CLAUDE.md`, used throughout, e.g. `crates/rl-save/src/unload.rs`'s `UnloadPlugin` needing `Saves`).
- Feature set is deliberately narrowed (see STACK.md) to exclude 3D, audio, animation, gizmos, scenes, picking and Bevy's own UI, since none of it applies to a glyph-terminal roguelike and all of it would bloat a wasm download.

## Data Storage

**Databases:**
- None. There is no SQL/NoSQL database anywhere in the workspace.

**Save Files / Persistence:**
- `crates/rl-save` defines a `SaveBackend` trait (`crates/rl-save/src/backend.rs`) with three implementations, selected per platform:
  - `FileBackend` - saves as `<slot>.save.ron` files beside the running executable (native default).
  - `MemoryBackend` - in-process `BTreeMap`, used in tests and for a run that must not touch disk.
  - `WebBackend` (`#[cfg(target_arch = "wasm32")]`) - one `localStorage` key per slot, prefixed `<name>:<slot>`.
- `Saves::platform_default(name)` picks `WebBackend` on wasm and `FileBackend` on every other target, so game code never branches on platform.
- Save payloads are RON, wrapped in a `Versioned` envelope (`crates/rl-save/src/versioned.rs`) whose version field lets a save from an older schema be rejected or migrated rather than silently misread.
- `crates/rl-save/src/morgue.rs` writes a plain-text death summary file alongside the save, meant to be opened in an editor, not reloaded.

**File Storage:**
- Local filesystem only, via `FileBackend` above and RON asset files loaded by each game (`examples/*/assets/*.ron`) and by `rl-render`'s tile-look files.
- No cloud storage (S3, GCS, etc.) anywhere in the workspace.

**Caching:**
- None beyond GitHub Actions' own build caches (see CI/CD below); no application-level cache layer.

## Browser Integration (wasm32 target)

**Local storage:**
- `web_sys::Window::local_storage()` is the only browser storage API used, accessed exclusively from `WebBackend` in `crates/rl-save/src/backend.rs`.

**Unload bridge:**
- `crates/rl-save/src/unload.rs`'s `UnloadPlugin` installs `pagehide` and `beforeunload` listeners via `web_sys`/`wasm_bindgen` so a closed browser tab still writes the last stashed save.
- The listener closure is intentionally leaked (`Closure::forget()`) because it must outlive the Bevy frame that installed it and run synchronously outside the app's own schedule; this is a one-off, deliberate exception, documented in the module's doc comment.
- On native targets the same `Stash` is flushed instead on `AppExit`, in `flush_on_exit`, so a window's close button also saves - the platform split is isolated to this one file.

**Rendering surface:**
- Bevy's own `webgl2` feature (see STACK.md) is how the glyph-grid renderer reaches a `<canvas>` in the browser; there is no custom WebGL/WebGPU code in this repo.

**No other browser APIs are touched:** no fetch/XHR, no WebSocket, no IndexedDB, no service worker, no cookies.

## Authentication and Identity

- None. There is no user account system, no login, no session, anywhere in the engine or the example games.

## Monitoring and Observability

**Error Tracking:**
- None (no Sentry/Bugsnag/etc.).

**Logs:**
- `bevy_log` (a named Bevy feature) provides `info!`/`warn!`/`error!` macros, used throughout for runtime diagnostics (e.g. `crates/rl-save/src/unload.rs` logs save-on-exit outcomes).
- No structured log shipping or external log aggregation.

## CI/CD and Deployment

**Source hosting:**
- GitHub, `github.com/rudehn/rl-engine` (from `[workspace.package] repository` in `Cargo.toml` and the README badges).

**CI pipeline (`.github/workflows/ci.yml`):**
- Triggers on push to `main` and on every pull request.
- Single `ubuntu-latest` job that: frees disk space (removes unrelated preinstalled toolchains), installs Bevy's Linux system libraries via `apt-get`, installs stable Rust with `clippy`, `rustfmt` and the `wasm32-unknown-unknown` target (`dtolnay/rust-toolchain@stable`), restores/saves the Cargo cache (`Swatinem/rust-cache@v2`), then runs in order: `cargo fmt --all --check`, `scripts/check-tiers.sh`, `scripts/check-guide.sh`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `scripts/check-template.sh` (generates and builds the `templates/starter` cargo-generate template), `cargo doc --workspace --no-deps` with `RUSTDOCFLAGS="-D warnings"` (fails on missing docs or broken doc-tests), `scripts/check-tiers.sh --wasm`, and `cargo bench --workspace --no-run`.

**Docs/guide deployment (`.github/workflows/pages.yml`):**
- Triggers on push to `main` and manual `workflow_dispatch`.
- `build` job: caches the wasm demo bundles keyed on tutorial source/assets/lockfile hashes (`actions/cache@v4`), installs `mdbook` 0.5.4 and `trunk` 0.21.14 by downloading pinned release tarballs, runs `scripts/build-demos.sh` to build each tutorial step to wasm and drop it into `docs/guide/src/demos/`, then `mdbook build docs/guide`.
- Before publishing, it probes `gh api repos/$GITHUB_REPOSITORY/pages`; if GitHub Pages is not enabled for the repo it logs a notice and skips the publish step entirely rather than failing the workflow (a deliberate stand-down, per the recent commit `4aed6a6 ci: the pages job stands down instead of failing`).
- If Pages is on: `actions/configure-pages@v5`, `actions/upload-pages-artifact@v3` (uploads `docs/guide/book`), and a `deploy` job using `actions/deploy-pages@v4`.
- Concurrency group `pages` with `cancel-in-progress: false` - deploys never overlap or get cut off mid-way.
- Published guide, when Pages is enabled: `rudehn.github.io/rl-engine`.

**Release workflow (`.github/workflows/release.yml`):**
- Triggers on pushing a tag matching `v*`.
- Verifies the pushed tag matches `[workspace.package] version` in `Cargo.toml` (fails the workflow otherwise, preventing a mistagged release).
- Publishes (or edits, if it already exists) a GitHub Release whose notes come from `scripts/release-notes.sh <version>`, sourced from the project changelog content.
- This workflow only manages the GitHub Release page; it does not run `cargo publish`.

**No other CI/CD:** no Docker builds, no container registry, no separate deploy-to-server step, no crates.io publish step yet (see below).

## Package Registry Status

**crates.io:**
- Not published. `README.md` states this directly: "rl-engine is not on crates.io yet, so depend on it from git," and shows a git+tag dependency instead (`rl-engine = { git = "...", tag = "v0.2.0" }`).
- `docs/TODO.md` (section "6. Publish it") documents two concrete blockers tracked for a future milestone:
  1. **Name collision** - `rl-core` collides with an existing, unrelated crate on crates.io, so the entire `rl-` prefix family needs a rename before publishing; candidate name families and the scope of the rename (297 references across 153 files) are recorded there.
  2. **Workspace path dependencies have no version requirement** - all 51 internal path dependencies flow through one `[workspace.dependencies]` table, so adding versions is mechanical, but 10 of 11 crates currently have no crate-level README (would render an empty crates.io page) and `rl-engine`'s `readme = "../../README.md"` points outside its own package, which `cargo package` rejects.
- `docs/TODO.md` also flags that the public API is still moving fast (77 commits touching public declarations in the 30 days before 2026-09-17) and recommends publishing the five Bevy-free crates first once the rename lands, keeping the Bevy-layer crates on a git dependency until their API settles.

## Distribution Mechanism (current)

**Template:**
- `cargo generate --git https://github.com/rudehn/rl-engine --tag v0.2.0 templates/starter --name my-game` - the supported way to start a new game today, in lieu of a crates.io crate.
- `scripts/check-template.sh` (run in CI) generates this template and builds it on every change, so it cannot silently rot.

**Direct git dependency:**
- Games depend on `rl-engine` (or a single tier-1 crate such as `rl-grid` for a Bevy-free tool) directly via `{ git = "https://github.com/rudehn/rl-engine", tag = "..." }` in their own `Cargo.toml`, per `README.md`.

## Webhooks and Callbacks

**Incoming:**
- None. No HTTP server exists anywhere in this codebase.

**Outgoing:**
- None, aside from the GitHub Actions workflows' own use of the `gh` CLI against the GitHub API (`gh api repos/.../pages`, `gh release create`/`gh release edit` in `release.yml` and `pages.yml`), which is CI/CD tooling rather than an application integration.

---

*Integration audit: 2026-09-17*
