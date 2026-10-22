# Soak results

Each row is one soak of `tests/soak.rs` (`cargo test -p undermere --test soak -- --ignored --nocapture`), at the commit it measured.
The latest row is the baseline; re-soak only when game code changed since its commit (see "Soaks" in `AGENTS.md`).

| Commit | Date | Seeds | Delves won | Stalled | Notes |
|---|---|---|---|---|---|
