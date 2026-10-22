# Soak results

Each row is one soak of `tests/soak.rs` (`cargo test -p undermere --test soak -- --ignored --nocapture`), at the commit it measured.
The latest row is the baseline; re-soak only when game code changed since its commit (see "Soaks" in `AGENTS.md`).

| Commit | Date | Seeds | Delves won | Stalled | Notes |
|---|---|---|---|---|---|
| `c1cf867e` | 2026-10-21 | 1-50 | 72 of 150 | 0 | Main before the first wave; same game code as `fbbbbe5c` and `72e866fa`. Died on floors 3-10: 3, 4, 9, 1, 6, 7, 22, 24. |
| the commit adding this row | 2026-10-21 | 1-50 | 70 of 150 | 0 | The palace grounds' first wave, the Royal Wood and the Palace Gardens on floors 3-4 with their landmarks. Died on floors 3-10: 5, 3, 10, 5, 6, 10, 12, 29; three of floor 3's five are one seed's polymorphed golem. Not retuned. |
| `fb458a1b` (the town wave alone on `fbbbbe5c`, not kept) | 2026-10-21 | 1-50 | 52 of 150 | 0 | The drowned town's first wave alone, the Old Town and the Harbour on floors 1-2 with their landmarks. Died on floors 3-10: 2, 11, 7, 6, 9, 13, 21, 29; none on 1-2. |
| `fb458a1b` (the town wave alone on `fbbbbe5c`, not kept) | 2026-10-22 | 51-100 | 64 of 150 | 0 | The same code on the next fifty seeds, against the strata's own 62 there: the first fifty's 52 read as the seeds rather than the floors. Died on floors 2-10: 2, 6, 7, 10, 5, 6, 3, 19, 28. |
| the commit adding this row | 2026-10-22 | 1-50 | 62 of 150 | 0 | Both halves of the first wave, the drowned town rebased onto the palace grounds. Died on floors 3-10: 2, 15, 6, 5, 9, 4, 13, 34; floor 4's fifteen are the palace floors' acolytes, scribes and pilgrims, against main's four and the strata's ten and a half a hundred and fifty over seeds 1-100. Not retuned. |
