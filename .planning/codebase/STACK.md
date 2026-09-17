# Technology Stack

**Analysis Date:** 2026-09-17

## Languages

**Primary:**
- Rust, edition 2024, MSRV 1.88 (`Cargo.toml` workspace package section) - every crate, example and template.

**Secondary:**
- RON (Rusty Object Notation) - all data-driven content: tiles, monsters, items, statuses, quests, abilities, and the tutorial's own genre fixtures.
  Example files: `examples/corsair/assets/monsters.ron`, `examples/delve/assets/abilities.ron`, `crates/rl-bevy/tests/genres/*.ron`.
- Shell (bash) - CI and developer scripts in `scripts/*.sh`.
- Python - `scripts/expand-guide.py`, which keeps guide chapters synced to the tutorial source they quote.
- Markdown - `docs/guide/src/*` (mdBook), `docs/PLAN.md`, `docs/OVERVIEW.md`, `docs/TODO.md`, `docs/reviews/`.

## Runtime

**Environment:**
- Native desktop (Linux, macOS, Windows via Bevy's winit backend) and `wasm32-unknown-unknown` in the browser.
- Tier 0 and tier 1 crates (`rl-core`, `rl-grid`, `rl-mapgen`, `rl-world`, `rl-rules`) and `rl-save` build for both targets; `scripts/check-tiers.sh --wasm` enforces this in CI.
- No `std::time::Instant` is permitted in the wasm-buildable crates (stated in `CLAUDE.md`), since it panics on `wasm32-unknown-unknown` without an extra shim.

**Package Manager:**
- Cargo, workspace resolver `"3"` (`Cargo.toml`).
- Lockfile: present, `Cargo.lock` committed at the repo root.

## Frameworks

**Core:**
- Bevy 0.19 - the ECS, app loop, windowing and rendering substrate for tier 2 and tier 3 crates.
  Pulled in with `default-features = false` and an explicit feature list in `[workspace.dependencies]` (`Cargo.toml`): `bevy_core_pipeline`, `bevy_sprite_render`, `bevy_text`, `bevy_window`, `bevy_winit`, `bevy_state`, `bevy_log`, `bevy_color`, `default_font`, `system_font_discovery`, `serialize`, `multi_threaded`, `png`, `webgl2`, `x11`, `wayland`, `std`, `async_executor`.
  The comment above the feature list explains why: default Bevy features carry 3D, glTF, audio, animation, gizmos, scenes, picking and Bevy's own UI widget set, none of which a glyph terminal renderer needs, and all of which a browser build would have to download.

**Testing:**
- The built-in `#[test]` harness (`cargo test`), used throughout every crate.
- `criterion` 0.5 (with `html_reports`) for benchmarks - `rl-grid` has `benches/grid.rs`; every algorithm crate is expected to carry benches per `README.md`'s design principles.
- Doc-tests: `#![deny(missing_docs)]` plus a rule that no example is ever fenced `ignore`, so every public doc comment's code sample compiles and runs as a test.

**Build/Dev:**
- `mdBook` 0.5.4 - builds the guide at `docs/guide`, pinned in `.github/workflows/pages.yml`.
- `trunk` 0.21.14 - builds the tutorial's wasm demo bundles embedded in the guide, pinned in `.github/workflows/pages.yml` and driven by `scripts/build-demos.sh`.
- `wasm-opt` (via trunk's `data-wasm-opt="z"`) - shrinks the demo wasm binaries for the browser.
- `cargo-generate` - the mechanism a user runs to instantiate `templates/starter` into a new game (`README.md`).
- `rustfmt` - `rustfmt.toml` pins `max_width = 160`, `use_small_heuristics = "Max"`, `edition = "2024"`; `cargo fmt --all --check` is enforced in CI.
- `clippy` - `cargo clippy --workspace --all-targets -- -D warnings` is enforced in CI; crate-wide `#![allow(clippy::too_many_arguments)]` is explicitly disallowed by `CLAUDE.md`.
- `jq` - used inside `scripts/check-tiers.sh` to parse `cargo metadata` output.

## Key Dependencies

**Critical:**
- `bevy` 0.19.0 (locked) - see Frameworks above; tier 2/3 only.
- `rand` 0.9 (workspace pin), with `default-features = false` at the workspace level because the default `getrandom` feature does not build for `wasm32-unknown-unknown` and engine code must never seed from OS entropy anyway.
  Individual crates that need a concrete generator opt back in to `std` and `std_rng` (e.g. `crates/rl-core/Cargo.toml`, `crates/rl-mapgen/Cargo.toml`).
  All engine randomness is required to flow through `RunSeed::derive(domain, index)` in `crates/rl-core/src/seed.rs`, backed by `rand::rngs::StdRng` chosen specifically because it is the same algorithm on every platform (a world seed must reproduce identical terrain in a browser and on desktop); `SmallRng` is explicitly rejected for that reason.
- `serde` 1 (with `derive`) - the serialization backbone for every RON-loaded registry and every save schema type.
- `ron` 0.10 - the on-disk/on-wire format for both content data and saves; `crates/rl-save` reads and writes `Versioned` RON envelopes (`crates/rl-save/src/versioned.rs`).
- `noise` 0.9 - FBM noise for world generation (`crates/rl-world`).

**Infrastructure:**
- `web-sys` 0.3 (feature-gated `Window`, `Storage`, `Event`, `EventTarget`) - browser `localStorage` and the `pagehide`/`beforeunload` listeners, wasm-only, `crates/rl-save/Cargo.toml` under `[target.'cfg(target_arch = "wasm32")'.dependencies]`.
- `wasm-bindgen` 0.2 - the JS interop glue for the same wasm-only save backend and unload bridge (`crates/rl-save/src/unload.rs`, `crates/rl-save/src/backend.rs`).
- `criterion` 0.5 - see Testing above.

**Locked transitive versions worth knowing (from `Cargo.lock`):**
- `bevy` 0.19.0, `rand` 0.9.5 (workspace-selected; 0.8.7 and 0.10.2 also appear in the lockfile as transitive deps of other crates, not used directly by rl-engine code), `ron` 0.10.1 (workspace-selected; 0.12.2 also present transitively), `serde` 1.0.229, `noise` 0.9.0, `criterion` 0.5.1, `web-sys` 0.3.103, `wasm-bindgen` 0.2.126.

## Configuration

**Environment:**
- No `.env` file or environment-based secrets in this repo; it is a game engine and example games, not a service.
- Runtime behavior is controlled by env vars read directly in game/example code, not `.env` files:
  - `RL_CAPTURE`, `RL_CAPTURE_KEYS`, `RL_CAPTURE_FRAMES`, `RL_CAPTURE_AT` - screenshot capture tooling, `crates/rl-render/src/capture.rs`.
  - `RL_RECORD`, `RL_REPLAY` - input recording/playback, `crates/rl-bevy/src/replay.rs`.
- `[package.metadata.rl-engine] tier = N` in every workspace member's `Cargo.toml` is the build's own configuration surface, read by `scripts/check-tiers.sh` via `cargo metadata` (never parsed from any other file, so a new or renamed crate cannot bypass the check).

**Build:**
- `Cargo.toml` (workspace root) - members, shared dependency versions, and two profile tweaks: `[profile.dev.package."*"] opt-level = 3` (dependencies always optimized) with `[profile.dev] opt-level = 1` (workspace crates get a light pass, since worldgen/pathfinding debug builds would otherwise take seconds per map); `[profile.bench] debug = true`.
- `rustfmt.toml` - formatting rules, see Frameworks/Build-Dev above.
- `docs/guide/book.toml` - mdBook config: navy theme, git edit links back to `docs/guide/{path}` on GitHub, playground execution disabled (`runnable = false`), section folding enabled.
- Each crate's `Cargo.toml` also carries `description`, `keywords`, `categories`, and inherits `license`, `repository`, `homepage`, `rust-version` from `[workspace.package]` - groundwork for eventual crates.io publishing (see INTEGRATIONS.md).

## Platform Requirements

**Development:**
- Rust stable toolchain with the `wasm32-unknown-unknown` target added.
- On Linux (including CI): `pkg-config`, `libasound2-dev`, `libudev-dev`, `libwayland-dev`, `libxkbcommon-dev`, `libx11-dev`, `libfontconfig1-dev` - Bevy's audio, input, windowing and font-discovery dependencies (`README.md`, `.github/workflows/ci.yml`).
- `mdbook`, `trunk`, `cargo-generate` needed only for guide-building, demo-building, and template-instantiation workflows respectively; not needed to build or run the engine or games.

**Production:**
- No server-side production deployment; the "product" is a library workspace plus example native/wasm games.
- Native builds run as ordinary desktop binaries (Linux/macOS/Windows) via winit.
- Web builds run as static wasm + JS/HTML bundles served from any static host; the guide's own demos are built by trunk and published via GitHub Pages (see INTEGRATIONS.md).

---

*Stack analysis: 2026-09-17*
