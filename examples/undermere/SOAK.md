# Soak results

Each row is one soak of `tests/soak.rs` (`cargo test -p undermere --test soak -- --ignored --nocapture`), at the commit it measured.
The latest row is the baseline; re-soak only when game code changed since its commit (see "Soaks" in `AGENTS.md`).

| Commit | Date | Seeds | Delves won | Stalled | Notes |
|---|---|---|---|---|---|
| `c1cf867e` | 2026-10-21 | 1-50 | 72 of 150 | 0 | Main before the first wave; same game code as `fbbbbe5c` and `72e866fa`. Died on floors 3-10: 3, 4, 9, 1, 6, 7, 22, 24. |
| the commit adding this row | 2026-10-21 | 1-50 | 70 of 150 | 0 | The palace grounds' first wave, the Royal Wood and the Palace Gardens on floors 3-4 with their landmarks. Died on floors 3-10: 5, 3, 10, 5, 6, 10, 12, 29; three of floor 3's five are one seed's polymorphed golem. Not retuned. |
