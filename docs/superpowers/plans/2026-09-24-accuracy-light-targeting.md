# Accuracy, light bands and the targeting box: implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Light gets three bands (dark, dim, lit) that shape stealth and a new to-hit roll for blows, shots and throws, and the targeting cursor shows the odds and every line behind them in a framed box at the bottom of the rail.

**Architecture:** The band rule is pure arithmetic in `rl-grid` (tier 1). The roll is a `HitModel` trait in `rl-rules` (tier 1) that turns plain facts into `Odds { hits, out_of, lines }`. `rl-bevy` gathers those facts through one `Marksmanship` system parameter that the resolvers, the targeting collector and the inspect collector all call, so what a panel shows is what the roll uses. `rl-ui` shows the odds in a boxed `TargetPanel`, in inspect, and names the player's band on the vitals strip.

**Tech Stack:** Rust 2024, Bevy ECS, `rand`, `serde`/`ron`, mdBook guide pages.

**Spec:** `docs/superpowers/specs/2026-09-24-accuracy-light-targeting-design.md`. Read it before starting; this plan argues from it.

## Global Constraints

- Never an em dash anywhere, code or prose; use a plain dash.
- Commit messages never carry a `Co-Authored-By` line naming an agent (the user's rule overrides the harness's reminder).
- Commit messages follow the repo's style: a lower-case sentence saying what is now true, as in `git log --oneline -10`.
- Every task's commit passes: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `scripts/check-tiers.sh`, `scripts/check-tiers.sh --wasm`, `scripts/check-overview.sh`, `python3 scripts/check-systems.py`, `scripts/check-guide.sh`.
- `rl-grid` and `rl-rules` are tier 1: no Bevy, build on `wasm32-unknown-unknown`, no `std::time::Instant`.
- `#![deny(missing_docs)]`: every public item gets a doc comment that says why, at the density of `crates/rl-core/src/turn.rs`.
- No `HashMap`/`HashSet` on gameplay paths. No `TODO` comments in source.
- Randomness comes only from `CombatRng` for the roll; `Certain` must draw nothing.
- Costs and chances are integers; `Odds` is `hits` in `out_of`, and `chance_pct` is a whole percent.
- A view holds no `Color`, no `Rect`, and no string the game did not supply; presenter words may have English defaults, as `Phrasebook` and `TargetPanel` do today.
- A system page (`docs/guide/src/systems/*.md`) whose manifest lists a file you changed must be re-read against the code, corrected, and blessed with `python3 scripts/check-systems.py --bless <system>`; then `scripts/check-systems-style.sh docs/guide/src/systems/<system>.md` must pass. Budget: 80 lines of prose.
- Guide code blocks are only `<!-- include: path:anchor -->` quotes of real `// ANCHOR: name` / `// ANCHOR_END: name` regions in `examples/`, fenced `rust,no_run`.
- `DEFAULT_BRIGHT` is 64. `Percent` clamps to 0..=100 with no floor or ceiling. `effective` defaults to `range / 3`, rounded down.
- Foundry's `Percent`: `per_tile: 5`, `dim: 16`, `dark: 30`.
- Commit after every task. The spec's slices map to tasks 1-2, 3-8, 9-11 and 12-13; each task's commit passes every check on its own.

## Review Focus

These are the inputs most likely to bite a player that no happy-path test exercises; each has its test in the owning task.

1. **The cursor on an empty cell, or on a wall**: the box says "Target: nothing", shows no chance row and does not panic (Task 9).
2. **A throw that a body in between intercepts**: the chance shown is against the body the flight strikes, at that body's distance, which is who the resolver rolls against (Task 9, and Task 5 for the resolver).
3. **A game with `Percent` and no `LightingPlugin`**: every target counts as lit, no light line appears, and the vitals strip shows no exposure word (Task 3 for the model, Task 2 for vitals).
4. **Firing point blank**: the cursor aimed at an adjacent foe shows the blow's odds, with no range or light line, since the resolver strikes a blow there (Task 9).
5. **A box taller than its rectangle**: more lines than rows are clipped inside the frame and nothing is drawn outside the rectangle (Task 10).

---

## File map

| File | Change | Responsibility |
|---|---|---|
| `crates/rl-grid/src/light.rs` | modify | `LightBand` and `LightBand::of` |
| `crates/rl-grid/src/lib.rs` | modify | export `LightBand` |
| `crates/rl-bevy/src/lighting.rs` | modify | `Lighting::bright`, `DEFAULT_BRIGHT`, `Lighting::band`, `band_at` |
| `crates/rl-bevy/src/stealth.rs` | modify | `lit` means the `Lit` band |
| `crates/rl-ui/src/view/vitals.rs`, `crates/rl-ui/src/panel/vitals.rs` | modify | `exposure` |
| `crates/rl-rules/src/accuracy.rs` | create | `Delivery`, `Line`, `Odds`, `Shot`, `HitModel`, `range_penalty`, `Certain`, `Percent`, `PercentLabels` |
| `crates/rl-rules/src/lib.rs` | modify | `pub mod accuracy` and exports |
| `crates/rl-bevy/src/accuracy.rs` | create | `HitRules`, `Missed`, `Attempt`, `Marksmanship` |
| `crates/rl-bevy/src/combat.rs` | modify | stats on `CombatRules`, `RangedAttack::effective`, misses in `resolve_attacks` and `land` |
| `crates/rl-bevy/src/throwing.rs` | modify | `Throwable::new`, `effective`, misses at landing |
| `crates/rl-bevy/src/lib.rs` | modify | `pub mod accuracy` and exports |
| `crates/rl-rules/src/forecast.rs` | modify | `Combatant::chance_pct`, `hitting` |
| `crates/rl-ui/src/narrate.rs` | modify | `YouMiss`, `MissesYou`, `OthersMiss` |
| `crates/rl-ui/src/view/inspect.rs`, `crates/rl-ui/src/panel/inspect.rs` | modify | `InspectView::odds` |
| `crates/rl-ui/src/view/target.rs` | modify | `TargetView::span`, `odds`, the weapon's name |
| `crates/rl-ui/src/panel/target.rs` | modify | the framed box replaces the banner |
| `examples/{foundry,corsair,heist,delve}/src/main.rs` | modify | the box at the bottom of the rail |
| `examples/foundry/src/{content.rs,gear.rs,plugin.rs}`, `examples/foundry/assets/items.ron` | modify | `Percent`, `effective`, the retune |
| every `Throwable { .. }` literal (listed in Task 4) | modify | `Throwable::new` |
| docs listed per task | modify / create | the documentation the change owes |

---

## Task 1: Light bands, and stealth reads the lit band

**Files:**
- Modify: `crates/rl-grid/src/light.rs` (add `LightBand` after `Light`'s impl, before `falloff`; tests in its `mod tests`)
- Modify: `crates/rl-grid/src/lib.rs:41` and `:53` (exports)
- Modify: `crates/rl-bevy/src/lighting.rs:85-146` (`DEFAULT_BRIGHT`, `bright`, `band`, `band_at`)
- Modify: `crates/rl-bevy/src/lib.rs:75` and the prelude line `134` (export `band_at`, `DEFAULT_BRIGHT`)
- Modify: `crates/rl-bevy/src/stealth.rs:302`
- Test: `crates/rl-grid/src/light.rs`, `crates/rl-bevy/src/stealth.rs` tests
- Docs: `docs/guide/src/systems/sight.md`, `docs/guide/src/systems/stealth.md`

**Interfaces:**
- Produces: `rl_grid::light::LightBand { Dark, Dim, Lit }` (also `rl_grid::LightBand`), `LightBand::of(intensity: u8, threshold: u8, bright: u8) -> LightBand`, `rl_bevy::lighting::DEFAULT_BRIGHT: u8 = 64`, `Lighting::bright: u8`, `Lighting::band(&self, p: Point) -> LightBand`, `rl_bevy::lighting::band_at(lighting: Option<&Lighting>, p: Point) -> LightBand`.

- [ ] **Step 1: Write the failing band test in `rl-grid`**

Add to `mod tests` in `crates/rl-grid/src/light.rs`:

```rust
    #[test]
    fn a_band_changes_exactly_at_the_threshold_and_at_bright() {
        for (threshold, bright) in [(16u8, 64u8), (1, 2), (0, 255), (40, 40)] {
            for intensity in 0..=255u8 {
                let band = LightBand::of(intensity, threshold, bright);
                let expected = if intensity < threshold {
                    LightBand::Dark
                } else if intensity < bright {
                    LightBand::Dim
                } else {
                    LightBand::Lit
                };
                assert_eq!(band, expected, "intensity {intensity} with threshold {threshold} and bright {bright}");
            }
        }
    }
```

- [ ] **Step 2: Run it and see it fail**

Run: `cargo test -p rl-grid a_band_changes_exactly`
Expected: compile error, `LightBand` not found.

- [ ] **Step 3: Add `LightBand`**

In `crates/rl-grid/src/light.rs`, after the `impl Light { .. }` block:

```rust
/// How lit a tile is, in the three steps gameplay reads.
///
/// Below the seen threshold a tile is dark and unseen; from there up to
/// `bright` it is dim, seen but hard to hit and easy to hide in; at or
/// above `bright` it is lit. Three steps rather than the intensity itself,
/// because a rule a player can learn is a word on a panel, and a number
/// that changes every step is not one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub enum LightBand {
    /// Below the seen threshold.
    Dark,
    /// Seen, and short of bright.
    Dim,
    /// At or above bright.
    Lit,
}

impl LightBand {
    /// The band `intensity` falls in, with `threshold` the least that is
    /// seen and `bright` the least that is lit. The one rule, so sight,
    /// stealth and a to-hit roll cannot draw the lines differently.
    pub fn of(intensity: u8, threshold: u8, bright: u8) -> LightBand {
        if intensity < threshold {
            LightBand::Dark
        } else if intensity < bright {
            LightBand::Dim
        } else {
            LightBand::Lit
        }
    }
}
```

In `crates/rl-grid/src/lib.rs` change both export lines `pub use light::{Emitter, Light, LightField, Rgb};` and the prelude's `pub use crate::light::{Emitter, Light, LightField, Rgb};` to include `LightBand`.

- [ ] **Step 4: Run the grid test and see it pass**

Run: `cargo test -p rl-grid a_band_changes_exactly`
Expected: PASS.

- [ ] **Step 5: Write the failing stealth test**

In `crates/rl-bevy/src/stealth.rs` `mod tests`, add a constructor option for light and the test. The `Field` struct's `new` gains no parameter; the test adds the plugin itself:

```rust
    /// A watcher whose certain radius only reaches the player in light.
    fn light_hunter() -> NoticeStats {
        NoticeStats { certain: 1, chance_pct: 0, lit_bonus: 10, memory: 3 }
    }

    /// The same field under an ambient of `intensity` and nothing else, so
    /// the band the player stands in is the ambient's.
    fn under(intensity: u8) -> Field {
        let mut field = Field::new(light_hunter(), 5, 10, true);
        field.app.add_plugins(crate::lighting::LightingPlugin);
        field.app.finish();
        field.app.world_mut().resource_mut::<crate::lighting::Lighting>().ambient = rl_grid::Light::white(intensity);
        field.app.update();
        field
    }

    #[test]
    fn a_watcher_gets_its_light_bonus_in_the_lit_band_and_not_in_the_dim_one() {
        // 40 is above the seen threshold of 16 and below bright at 64: the
        // player is seen but dim, so the bonus of ten does not reach five
        // tiles and a watcher that never rolls a chance never notices.
        let mut dim = under(40);
        for _ in 0..3 {
            dim.wait();
        }
        assert!(!dim.aware().knows(dim.player), "dim light hides the player from a watcher that sees by light");

        let mut lit = under(100);
        lit.wait();
        assert!(lit.aware().knows(lit.player), "in the lit band the bonus of ten covers five tiles");
    }
```

If adding `LightingPlugin` after `Field::new` has already entered play trips a plugin-order assertion, move the plugin into `Field::new` behind a new `light: Option<u8>` argument instead, and pass `None` from every existing call. Keep the test's assertions unchanged.

- [ ] **Step 6: Run it and see it fail**

Run: `cargo test -p rl-bevy a_watcher_gets_its_light_bonus`
Expected: FAIL on the first assertion, because `is_lit` counts 40 as lit today.

- [ ] **Step 7: Add `bright`, `band` and `band_at` to `Lighting`**

In `crates/rl-bevy/src/lighting.rs`, below `DEFAULT_THRESHOLD`:

```rust
/// The intensity at or above which a seen tile counts as lit rather than
/// dim, unless the game says otherwise. With the lamps the example games
/// carry, it leaves a ring of one or two tiles of dim light at a lamp's
/// edge, which is where a sneak stands and where a shot goes wide.
pub const DEFAULT_BRIGHT: u8 = 64;
```

Add the field to `Lighting`, after `threshold`:

```rust
    /// The intensity at or above which a seen tile is lit rather than dim.
    pub bright: u8,
```

Set `bright: DEFAULT_BRIGHT,` in `Lighting::new`. Add after `is_lit`:

```rust
    /// The band world tile `p` is in, by `threshold` and `bright`.
    pub fn band(&self, p: Point) -> LightBand {
        LightBand::of(self.at(p).intensity, self.threshold, self.bright)
    }
```

And a free function after `perceives`:

```rust
/// The band `p` is in, with no lighting read as lit: a game without
/// [`LightingPlugin`] is a world seen everywhere, and nothing in it is
/// harder to hit or easier to hide in for want of light.
pub fn band_at(lighting: Option<&Lighting>, p: Point) -> LightBand {
    lighting.map_or(LightBand::Lit, |l| l.band(p))
}
```

Import `rl_grid::LightBand` at the top of the file, and re-export it from `crates/rl-bevy/src/lib.rs` beside the lighting line: `pub use lighting::{DarkSight, Fuel, LightEvent, LightSource, Lighting, LightingPlugin, band_at};` and `pub use rl_grid::LightBand;` (and the same two in the prelude block at line 134).

- [ ] **Step 8: Make stealth read the band**

In `crates/rl-bevy/src/stealth.rs:302` replace

```rust
            let lit = lighting.is_none_or(|l| l.is_lit(at.0));
```

with

```rust
            // Lit means the lit band, not merely seen: a subject in a
            // lamp's dim ring is seen, and is harder to pick out there.
            let lit = crate::lighting::band_at(lighting, at.0) == rl_grid::LightBand::Lit;
```

- [ ] **Step 9: Run the stealth tests and the whole crate**

Run: `cargo test -p rl-bevy stealth` then `cargo test --workspace`
Expected: all PASS. If a game test in `examples/` fails because a watcher no longer notices a player in dim light, read the test: if it pinned the old behaviour deliberately, update its expectation and say why in its comment; if it was only using light to get noticed, give it ambient light at or above 64.

- [ ] **Step 10: Update the two guide pages**

`docs/guide/src/systems/sight.md`, `The model`: after the sentence about `threshold` starting at `DEFAULT_THRESHOLD`, add:

```markdown
`bright` starts at `DEFAULT_BRIGHT`, which is 64, and `band` names a tile's `LightBand`: `Dark` below `threshold`, `Dim` from there to `bright`, and `Lit` at or above it.
`band_at` reads a missing `Lighting` as lit, the same way a game without lighting sees everywhere.
```

Add `crates/rl-grid/src/light.rs` is already in its manifest; confirm. In `The line`, add one sentence: `What the bands are worth is the game's: the engine names them, stealth reads the lit band for its bonus, and a to-hit model may read either.`

`docs/guide/src/systems/stealth.md`: change "a subject standing in light widens the observer's certain radius by `lit_bonus`" to "a subject standing in the lit band widens the observer's certain radius by `lit_bonus`, and one in a lamp's dim ring does not". Add `crates/rl-grid/src/light.rs` to its manifest `files:`.

Run: `python3 scripts/check-systems.py --bless sight` and `python3 scripts/check-systems.py --bless stealth`, then `scripts/check-systems-style.sh docs/guide/src/systems/sight.md` and the same for `stealth.md`.
Expected: all pass.

- [ ] **Step 11: Changelog**

Under `## Unreleased` in `CHANGELOG.md` (create the heading if it is not there, following the file's existing format), add:

```markdown
- Light has three bands: `Lighting::bright` (default 64) splits what is seen into `Dim` and `Lit`, read with `Lighting::band` or `band_at`.
- Stealth's `lit_bonus` now applies only in the `Lit` band. A subject in a lamp's dim ring, or under an ambient between `threshold` and `bright`, no longer gives it. A game that wants the old reading sets `bright` equal to `threshold`.
```

- [ ] **Step 12: Run every check and commit**

Run the full list in Global Constraints.

```bash
git add crates/rl-grid crates/rl-bevy docs/guide/src/systems/sight.md docs/guide/src/systems/stealth.md CHANGELOG.md
git commit -m "light has three bands, and stealth's light bonus reads the lit one"
```

---

## Task 2: The vitals strip names the player's exposure

**Files:**
- Modify: `crates/rl-ui/src/view/vitals.rs` (field, collector, test)
- Modify: `crates/rl-ui/src/panel/vitals.rs:144-150` and `:234-237` (both layouts, test)
- Docs: `docs/guide/src/systems/panels.md`, `docs/OVERVIEW.md`

**Interfaces:**
- Consumes: `band_at`, `LightBand` from Task 1.
- Produces: `VitalsView::exposure: Option<LightBand>`.

- [ ] **Step 1: Write the failing view test**

In `crates/rl-ui/src/view/vitals.rs` tests:

```rust
    #[test]
    fn exposure_is_the_band_the_player_stands_in_and_none_without_lighting() {
        let mut stage = Stage::new(VitalsViewPlugin);
        stage.tick();
        assert_eq!(stage.app.world().resource::<VitalsView>().exposure, None, "no lighting, nothing to say");

        let mut stage = Stage::new((VitalsViewPlugin, rl_bevy::LightingPlugin));
        stage.app.world_mut().resource_mut::<Lighting>().ambient = rl_grid::Light::white(40);
        stage.tick();
        stage.tick();
        assert_eq!(stage.app.world().resource::<VitalsView>().exposure, Some(LightBand::Dim));
        stage.app.world_mut().resource_mut::<Lighting>().ambient = rl_grid::Light::white(200);
        stage.tick();
        stage.tick();
        assert_eq!(stage.app.world().resource::<VitalsView>().exposure, Some(LightBand::Lit));
    }
```

- [ ] **Step 2: Run it and see it fail**

Run: `cargo test -p rl-ui exposure_is_the_band`
Expected: compile error, no field `exposure`.

- [ ] **Step 3: Add the field and fill it**

Field, after `seen`:

```rust
    /// The band of light the player stands in, which is what a watcher's
    /// light bonus and a shot at the player read. `None` in a game without
    /// lighting, where every tile is lit and the word would say nothing.
    pub exposure: Option<LightBand>,
```

Add `lighting: Option<Res<'w, Lighting>>,` to `Me`. In `collect_vitals`, after `view.seen = ...`:

```rust
    view.exposure = me.lighting.as_deref().map(|l| l.band(pos.0));
```

- [ ] **Step 4: Run it and see it pass**

Run: `cargo test -p rl-ui exposure_is_the_band`
Expected: PASS.

- [ ] **Step 5: Write the failing panel test**

In `crates/rl-ui/src/panel/vitals.rs` tests:

```rust
    #[test]
    fn the_band_the_player_stands_in_is_named_beside_whether_it_has_been_seen() {
        let mut stage =
            Stage::new((VitalsPanel::new(Rect::new(0, 0, 24, 8)), rl_bevy::MindsPlugin, rl_bevy::StealthPlugin, rl_bevy::LightingPlugin)).screen(24, 8);
        let player = stage.player;
        stage.app.world_mut().entity_mut(player).insert(rl_bevy::Stealth::default());
        stage.app.world_mut().resource_mut::<rl_bevy::Lighting>().ambient = rl_grid::Light::white(40);
        stage.tick();
        stage.tick();
        assert!(stage.rows().contains(&"hidden  dim".to_string()), "{:?}", stage.rows());
    }

    #[test]
    fn a_player_that_cannot_hide_is_still_told_its_exposure() {
        let mut stage = Stage::new((VitalsPanel::new(Rect::new(0, 0, 24, 8)), rl_bevy::LightingPlugin)).screen(24, 8);
        stage.app.world_mut().resource_mut::<rl_bevy::Lighting>().ambient = rl_grid::Light::white(200);
        stage.tick();
        stage.tick();
        assert!(stage.rows().contains(&"lit".to_string()), "{:?}", stage.rows());
    }
```

- [ ] **Step 6: Run and see them fail**

Run: `cargo test -p rl-ui the_band_the_player_stands_in a_player_that_cannot_hide`
Expected: FAIL, the rows hold `hidden` alone.

- [ ] **Step 7: Draw it in both layouts**

Add a helper in `crates/rl-ui/src/panel/vitals.rs`:

```rust
/// The word and tone for the band the player stands in: dark is good news
/// for a sneak and lit is bad, the same reading as seen and hidden.
fn exposure_word(band: LightBand) -> (&'static str, ToneId) {
    match band {
        LightBand::Dark => ("dark", Tones::GOOD),
        LightBand::Dim => ("dim", Tones::NOTICE),
        LightBand::Lit => ("lit", Tones::BAD),
    }
}
```

Replace the tall layout's `seen` block (lines 144-150) with one that prints the seen word, then two spaces, then the band word, each in its own tone, on one line, and prints the band alone when `seen` is `None`:

```rust
    let seen_word = view.seen.map(|seen| if seen { ("seen", Tones::BAD) } else { ("hidden", Tones::GOOD) });
    let band_word = view.exposure.map(exposure_word);
    if (seen_word.is_some() || band_word.is_some()) && y < bottom {
        let mut x = rect.x;
        for (i, (word, tone)) in seen_word.into_iter().chain(band_word).enumerate() {
            if i > 0 {
                x += 2;
            }
            terminal.print_on(x, y, word, palette.get(tone), bg);
            x += word.chars().count() as i32;
        }
        y += 1;
    }
```

In `draw_line`, after the `seen` part, push the band as its own part:

```rust
    if let Some(band) = view.exposure {
        let (word, tone) = exposure_word(band);
        parts.push(Part::Text(word.into(), palette.get(tone)));
    }
```

- [ ] **Step 8: Run the vitals tests**

Run: `cargo test -p rl-ui vitals`
Expected: all PASS, including the existing `a_player_that_can_hide_is_told_whether_it_has_been_seen` (no lighting there, so `hidden` stands alone).

- [ ] **Step 9: Docs**

`docs/guide/src/systems/panels.md`: where the vitals strip is described, add: `With lighting on, the strip names the band the player stands in, dark, dim or lit, beside seen or hidden, since the same band decides how well a watcher sees it and how hard it is to hit.` Bless `panels` if its manifest lists `crates/rl-ui/src/view/vitals.rs` or the panel file (check the manifest; if neither is listed and the page makes this claim, add both).
`docs/OVERVIEW.md`: in the `rl-ui` presenter table's vitals row, add "and the light band the player stands in".

- [ ] **Step 10: Run every check and commit**

```bash
git add crates/rl-ui docs/guide/src/systems/panels.md docs/OVERVIEW.md
git commit -m "the vitals strip says whether the player stands in the dark, dim or lit"
```

---

## Task 3: The odds, as arithmetic in `rl-rules`

**Files:**
- Create: `crates/rl-rules/src/accuracy.rs`
- Modify: `crates/rl-rules/src/lib.rs` (module list at line 46-62, module docs naming every public item, re-exports, prelude)

**Interfaces:**
- Consumes: `rl_grid::LightBand` from Task 1.
- Produces, all in `rl_rules::accuracy`:
  - `enum Delivery { Melee, Shot, Thrown }`
  - `struct Line { pub label: String, pub value: i32 }`
  - `struct Odds { pub hits: u32, pub out_of: u32, pub lines: Vec<Line> }` with `Odds::new(hits: u32, out_of: u32, lines: Vec<Line>) -> Odds`, `percent(&self) -> u32`, `roll(&self, rng: &mut impl Rng) -> bool`
  - `struct Shot { pub delivery: Delivery, pub distance: i32, pub effective: i32, pub range: i32, pub light: LightBand, pub accuracy: Option<i32>, pub evasion: Option<i32> }`
  - `trait HitModel: Send + Sync { fn odds(&self, shot: &Shot) -> Option<Odds>; }`
  - `fn range_penalty(distance: i32, effective: i32, per_tile: i32) -> i32`
  - `struct Certain;`
  - `struct Percent { pub per_tile: i32, pub dim: i32, pub dark: i32, pub labels: PercentLabels }`, `Percent::new(per_tile: i32, dim: i32, dark: i32) -> Percent`, `Percent::labelled(self, labels: PercentLabels) -> Percent`
  - `struct PercentLabels { pub accuracy: String, pub evasion: String, pub range: String, pub dim: String, pub dark: String }` with `Default`

- [ ] **Step 1: Write the tests against stubbed bodies**

Create `crates/rl-rules/src/accuracy.rs` holding the type and trait definitions from Step 3 with every function body `unimplemented!()`, and this test module. The stubs exist only so the tests compile and fail; Step 3 replaces them, and none reaches a commit.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rand::{SeedableRng, rngs::StdRng};

    fn shot(delivery: Delivery, distance: i32, light: LightBand) -> Shot {
        Shot { delivery, distance, effective: 3, range: 9, light, accuracy: None, evasion: None }
    }

    #[test]
    fn range_costs_nothing_inside_effective_and_exactly_per_tile_past_it() {
        for effective in 0..8 {
            for per_tile in 0..10 {
                for distance in 0..20 {
                    let expected = if distance <= effective { 0 } else { (distance - effective) * per_tile };
                    assert_eq!(range_penalty(distance, effective, per_tile), expected);
                }
            }
        }
    }

    #[test]
    fn percent_is_always_a_probability_whatever_it_is_fed() {
        let model = Percent::new(5, 16, 30);
        for accuracy in [None, Some(-50), Some(0), Some(60), Some(100), Some(250)] {
            for evasion in [None, Some(-40), Some(0), Some(35), Some(300)] {
                for distance in 0..30 {
                    for light in [LightBand::Dark, LightBand::Dim, LightBand::Lit] {
                        for delivery in [Delivery::Melee, Delivery::Shot, Delivery::Thrown] {
                            let odds = model.odds(&Shot { accuracy, evasion, ..shot(delivery, distance, light) }).expect("percent always rolls");
                            assert_eq!(odds.out_of, 100);
                            assert!(odds.hits <= 100);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn a_blow_reads_no_range_and_no_light_and_a_shot_reads_both() {
        let model = Percent::new(5, 16, 30);
        let blow = model.odds(&shot(Delivery::Melee, 1, LightBand::Dark)).unwrap();
        assert_eq!((blow.hits, blow.lines.len()), (100, 0), "a blow at accuracy 100 is certain and says nothing");

        let far = model.odds(&shot(Delivery::Shot, 5, LightBand::Dim)).unwrap();
        assert_eq!(far.hits, 100 - 10 - 16);
        let lines: Vec<(i32, &str)> = far.lines.iter().map(|l| (l.value, l.label.as_str())).collect();
        assert_eq!(lines, vec![(-10, "past effective range"), (-16, "for dim light")]);
    }

    #[test]
    fn accuracy_and_evasion_are_lines_only_when_named() {
        let model = Percent::new(5, 16, 30);
        let named = model.odds(&Shot { accuracy: Some(80), evasion: Some(10), ..shot(Delivery::Shot, 2, LightBand::Lit) }).unwrap();
        assert_eq!(named.hits, 70);
        let lines: Vec<i32> = named.lines.iter().map(|l| l.value).collect();
        assert_eq!(lines, vec![-20, -10], "accuracy against 100, then evasion");
    }

    #[test]
    fn certain_never_rolls() {
        for distance in 0..20 {
            assert!(Certain.odds(&shot(Delivery::Shot, distance, LightBand::Dark)).is_none());
        }
    }

    #[test]
    fn a_roll_hits_about_as_often_as_the_odds_say() {
        for hits in [0u32, 1, 25, 67, 99, 100] {
            let odds = Odds::new(hits, 100, Vec::new());
            let mut rng = StdRng::seed_from_u64(u64::from(hits) + 7);
            let landed = (0..20_000).filter(|_| odds.roll(&mut rng)).count() as f64 / 20_000.0;
            let expected = f64::from(hits) / 100.0;
            assert!((landed - expected).abs() < 0.015, "{hits} in 100 landed {landed}");
        }
    }

    #[test]
    fn percent_rounds_and_never_divides_by_zero() {
        assert_eq!(Odds::new(13, 20, Vec::new()).percent(), 65);
        assert_eq!(Odds::new(1, 3, Vec::new()).percent(), 33);
        assert_eq!(Odds::new(2, 3, Vec::new()).percent(), 67);
        assert_eq!(Odds::new(9, 0, Vec::new()).out_of, 1, "an empty denominator is one, not a panic");
        assert_eq!(Odds::new(9, 4, Vec::new()).hits, 4, "never more hits than outcomes");
    }

    /// A d20 model: d20 + accuracy against 10 + evasion, a natural 1
    /// always missing and a 20 always hitting. Written here to prove the
    /// trait is not a percent trait in disguise.
    struct D20;

    impl HitModel for D20 {
        fn odds(&self, shot: &Shot) -> Option<Odds> {
            let bonus = shot.accuracy.unwrap_or(0) - if shot.light == LightBand::Dim { 2 } else { 0 };
            let needed = 10 + shot.evasion.unwrap_or(0) - bonus;
            let faces = (21 - needed).clamp(1, 19) as u32;
            let mut lines = Vec::new();
            if shot.light == LightBand::Dim {
                lines.push(Line { label: "for dim light".into(), value: -2 });
            }
            Some(Odds::new(faces, 20, lines))
        }
    }

    #[test]
    fn a_d20_model_fits_the_trait_with_its_exact_chance() {
        let odds = D20.odds(&Shot { accuracy: Some(3), evasion: Some(2), ..shot(Delivery::Shot, 4, LightBand::Dim) }).unwrap();
        // Needs 10 + 2 - (3 - 2) = 11 or better: ten faces of twenty.
        assert_eq!((odds.hits, odds.out_of, odds.percent()), (10, 20, 50));
        assert_eq!(odds.lines[0].value, -2);
    }
}
```

Add `pub mod accuracy;` to `crates/rl-rules/src/lib.rs` in alphabetical position (before `affix`), and `pub use accuracy::{Certain, Delivery, HitModel, Line, Odds, Percent, PercentLabels, Shot, range_penalty};`. Name the module in the crate's `//!` docs where every module is listed.

- [ ] **Step 2: Run and see them fail**

Run: `cargo test -p rl-rules accuracy`
Expected: FAIL, panics at `unimplemented!`.

- [ ] **Step 3: Write the module**

Replace the stubs with the real module, keeping the tests:

```rust
//! Whether an attack lands: the odds, and the one roll that settles them.
//!
//! The engine gathers the facts, who is attacking, how far, in what light,
//! with what accuracy against what evasion, into a [`Shot`], and a
//! [`HitModel`] the game chose turns them into [`Odds`]: some number of
//! chances in some number of outcomes, and the labelled [`Line`]s that
//! produced them. Every model reduces to that, a percent roll, a d20
//! against a target number, two dice against each other, so a panel can
//! print a chance and a list of reasons whatever the model is, and the
//! resolver rolls one draw whatever the model is.
//!
//! [`Certain`] is the default and never rolls, so a game that has not
//! chosen accuracy draws nothing and plays as it did. [`Percent`] is the
//! one the engine ships: accuracy less evasion, less a penalty per tile
//! past the weapon's effective range, less a penalty for dim or dark light
//! at the target. No floor and no ceiling beyond 0 and 100: a game that
//! wants "never certain, never hopeless" writes that into its own model.

use rand::Rng;
use rl_grid::LightBand;

/// How an attack travels, which decides what distance and light mean to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Delivery {
    /// Struck in reach. Distance and light are not read: a blow lands on
    /// the cell beside the attacker, which the adjacency floor always shows.
    Melee,
    /// Fired down a line.
    Shot,
    /// Thrown.
    Thrown,
}

/// One labelled contribution to a chance, in the model's own units:
/// percentage points for [`Percent`], steps of one on a die for a d20
/// model. What a panel prints under the chance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// What the game called it.
    pub label: String,
    /// How much it moved the chance, negative for worse.
    pub value: i32,
}

/// The chance an attack lands: `hits` of `out_of` equally likely outcomes,
/// and what shaped it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Odds {
    /// Outcomes that hit.
    pub hits: u32,
    /// Outcomes there are. Never zero.
    pub out_of: u32,
    /// What moved the chance, in the order the model counted it.
    pub lines: Vec<Line>,
}

impl Odds {
    /// `hits` in `out_of`, with `out_of` at least one and `hits` at most
    /// `out_of`, so a model that miscounts gives a certain hit or miss
    /// rather than a panic in the middle of a turn.
    pub fn new(hits: u32, out_of: u32, lines: Vec<Line>) -> Odds {
        let out_of = out_of.max(1);
        Odds { hits: hits.min(out_of), out_of, lines }
    }

    /// The chance as a whole percent, rounded to nearest, for a panel.
    pub fn percent(&self) -> u32 {
        (self.hits * 100 + self.out_of / 2) / self.out_of
    }

    /// One draw: whether this attack lands.
    pub fn roll(&self, rng: &mut impl Rng) -> bool {
        rng.random_range(0..self.out_of) < self.hits
    }
}

/// The facts a [`HitModel`] reads, plain data gathered by the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shot {
    /// How the attack travels.
    pub delivery: Delivery,
    /// Chebyshev distance from attacker to target.
    pub distance: i32,
    /// The furthest distance with no range penalty. Zero for a blow.
    pub effective: i32,
    /// The furthest the attack reaches. One for a blow.
    pub range: i32,
    /// The light at the target.
    pub light: LightBand,
    /// The attacker's accuracy stat, `None` when the game names none.
    pub accuracy: Option<i32>,
    /// The target's evasion stat, `None` when the game names none.
    pub evasion: Option<i32>,
}

/// A way of turning a [`Shot`] into [`Odds`]. The game's choice, held by
/// `rl-bevy`'s `HitRules`.
pub trait HitModel: Send + Sync {
    /// The odds of `shot` landing, or `None` for an attack that is not
    /// rolled at all and simply lands.
    fn odds(&self, shot: &Shot) -> Option<Odds>;
}

/// What range costs: nothing at or inside `effective`, and `per_tile` for
/// every tile past it.
pub fn range_penalty(distance: i32, effective: i32, per_tile: i32) -> i32 {
    (distance - effective).max(0) * per_tile
}

/// Every attack lands and nothing is drawn: the default, and how every game
/// played before accuracy existed.
#[derive(Debug, Clone, Copy, Default)]
pub struct Certain;

impl HitModel for Certain {
    fn odds(&self, _: &Shot) -> Option<Odds> {
        None
    }
}

/// What [`Percent`] calls each of its lines. English by default, as the
/// narrator's phrasebook is; a game says its own with [`Percent::labelled`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PercentLabels {
    /// The attacker's accuracy, against 100.
    pub accuracy: String,
    /// The target's evasion.
    pub evasion: String,
    /// The range penalty.
    pub range: String,
    /// Dim light at the target.
    pub dim: String,
    /// Dark at the target, seen only by dark sight.
    pub dark: String,
}

impl Default for PercentLabels {
    fn default() -> Self {
        Self {
            accuracy: "accuracy".into(),
            evasion: "evasion".into(),
            range: "past effective range".into(),
            dim: "for dim light".into(),
            dark: "in the dark".into(),
        }
    }
}

/// A percent roll: accuracy (100 when the game names no stat) less
/// evasion (0 when it names none), less [`range_penalty`], less `dim` or
/// `dark` for the light at the target, clamped to 0..=100. A blow reads
/// neither range nor light.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Percent {
    /// Points lost per tile past effective range.
    pub per_tile: i32,
    /// Points lost for a target in dim light.
    pub dim: i32,
    /// Points lost for a target in the dark.
    pub dark: i32,
    /// What each line is called.
    pub labels: PercentLabels,
}

impl Percent {
    /// The model with these penalties and the English labels.
    pub fn new(per_tile: i32, dim: i32, dark: i32) -> Percent {
        Percent { per_tile, dim, dark, labels: PercentLabels::default() }
    }

    /// The same, with the game's own words for its lines.
    pub fn labelled(mut self, labels: PercentLabels) -> Percent {
        self.labels = labels;
        self
    }
}

impl HitModel for Percent {
    fn odds(&self, shot: &Shot) -> Option<Odds> {
        let mut lines = Vec::new();
        let mut push = |label: &str, value: i32| {
            if value != 0 {
                lines.push(Line { label: label.to_string(), value });
            }
        };
        let accuracy = shot.accuracy.unwrap_or(100);
        push(&self.labels.accuracy, accuracy - 100);
        push(&self.labels.evasion, -shot.evasion.unwrap_or(0));
        if shot.delivery != Delivery::Melee {
            push(&self.labels.range, -range_penalty(shot.distance, shot.effective, self.per_tile));
            match shot.light {
                LightBand::Lit => {}
                LightBand::Dim => push(&self.labels.dim, -self.dim),
                LightBand::Dark => push(&self.labels.dark, -self.dark),
            }
        }
        let total = 100 + lines.iter().map(|l| l.value).sum::<i32>();
        Some(Odds::new(total.clamp(0, 100) as u32, 100, lines))
    }
}
```

- [ ] **Step 4: Run the tests and the wasm tier check**

Run: `cargo test -p rl-rules accuracy` then `scripts/check-tiers.sh --wasm`
Expected: PASS for both.

- [ ] **Step 5: Run every check and commit**

```bash
git add crates/rl-rules
git commit -m "the odds of an attack landing are a model a game chooses, with percent and certain shipped"
```

---

## Task 4: The facts, gathered in `rl-bevy`

**Files:**
- Create: `crates/rl-bevy/src/accuracy.rs`
- Modify: `crates/rl-bevy/src/combat.rs` (`CombatRules` fields and builders at 212-275, `RangedAttack` at 140-179, `CombatPlugin::build` at 905-928)
- Modify: `crates/rl-bevy/src/throwing.rs:33-43` (`Throwable`)
- Modify: `crates/rl-bevy/src/lib.rs` (module, exports, prelude)
- Modify, mechanically, every `Throwable { range: .., strike: .. }` literal to `Throwable::new(range, strike)`:
  `crates/rl-ui/src/narrate.rs:1314`, `crates/rl-ui/src/panel/inventory.rs:454`, `:640`, `crates/rl-ui/src/view/inventory.rs:264`, `crates/rl-ui/src/view/target.rs:1008`, `crates/rl-bevy/src/throwing.rs:288`, `:315`, `:453`, `crates/rl-bevy/src/minds.rs:978`, `examples/tutorial/src/bin/step04_things.rs:409`, `step05_knack.rs:452`, `step06_descent.rs:505`, `examples/foundry/src/gear.rs:344`, `examples/corsair/src/items.rs:325`, `examples/heist/src/main.rs:441`. Re-run `grep -rn "Throwable {" crates examples` to catch any added since.
- Docs: `docs/guide/src/systems/combat.md` (manifest and `The model`), `docs/guide/src/systems/items.md` if its manifest lists `throwing.rs`

**Interfaces:**
- Consumes: `rl_rules::accuracy::*` from Task 3; `band_at` from Task 1.
- Produces:
  - `CombatRules::accuracy: Option<StatId>`, `CombatRules::evasion: Option<StatId>`, `accuracy_stat(self, StatId) -> Self`, `evasion_stat(self, StatId) -> Self`
  - `RangedAttack::effective: Option<i32>`, `RangedAttack::effective_to(self, i32) -> Self`, `RangedAttack::effective_range(&self) -> i32`
  - `Throwable::effective: Option<i32>`, `Throwable::new(range: i32, strike: Option<(DamageKindId, DiceRoll)>) -> Throwable`, `effective_to(self, i32) -> Self`, `effective_range(&self) -> i32`
  - `rl_bevy::accuracy::HitRules(pub Box<dyn HitModel>)`, `Default` is `Certain`
  - `rl_bevy::accuracy::Missed { pub attacker: Entity, pub target: Entity, pub with: Option<Entity>, pub reach: Reach }` (a `Message`)
  - `rl_bevy::accuracy::Attempt<'a> { Blow, Shot(&'a RangedAttack), Throw(&'a Throwable) }`
  - `rl_bevy::accuracy::Marksmanship` (`SystemParam`) with `odds(&self, attacker: Entity, target: Entity, attempt: Attempt) -> Option<Odds>` and `at_distance(&self, loadout: &Loadout, attacker: Entity, target: Entity) -> Option<Odds>`

- [ ] **Step 1: Write the failing tests**

At the bottom of the new `crates/rl-bevy/src/accuracy.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::{CombatPlugin, Health, MeleeAttack, RangedAttack};
    use crate::components::{Actor, Blocks, Position};
    use crate::plugin::headless_app;
    use crate::state::EngineState;
    use rl_core::DiceRoll;
    use rl_rules::accuracy::Percent;

    /// What a system asked of `Marksmanship` for `attempt` from the
    /// shooter at the target, read back out of the world.
    #[derive(Resource, Default)]
    struct Asked(Option<Odds>);

    #[derive(Resource, Clone, Copy)]
    struct Pair(Entity, Entity);

    fn ask_shot(marks: Marksmanship, pair: Res<Pair>, guns: Query<&RangedAttack>, mut out: ResMut<Asked>) {
        let gun = guns.get(pair.0).expect("the shooter has a gun");
        out.0 = marks.odds(pair.0, pair.1, Attempt::Shot(gun));
    }

    fn range(gap: i32, rules: Option<HitRules>, lighting: Option<u8>) -> App {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, CombatPlugin, crate::world::StreamingPlugin));
        if lighting.is_some() {
            app.add_plugins(crate::lighting::LightingPlugin);
        }
        let start = crate::testing::surface(&mut app);
        let sides = crate::testing::two_sides(&mut app);
        if let Some(rules) = rules {
            app.insert_resource(rules);
        }
        let gun = RangedAttack::new(sides.kind, DiceRoll::flat(1), 12).effective_to(3);
        let shooter = app.world_mut().spawn((Actor, Blocks, Position(start), Health::full(10), gun, MeleeAttack::new(sides.kind, DiceRoll::flat(1)))).id();
        let target = app.world_mut().spawn((Actor, Blocks, Position(start.offset(gap, 0)), Health::full(10))).id();
        app.insert_resource(Pair(shooter, target)).init_resource::<Asked>().add_systems(PostUpdate, ask_shot);
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        if let Some(intensity) = lighting {
            app.world_mut().resource_mut::<crate::lighting::Lighting>().ambient = rl_grid::Light::white(intensity);
        }
        app.update();
        app.update();
        app
    }

    #[test]
    fn with_no_model_chosen_nothing_is_rolled() {
        let app = range(5, None, None);
        assert_eq!(app.world().resource::<Asked>().0, None);
    }

    #[test]
    fn the_facts_reach_the_model_distance_past_effective_and_the_light_at_the_target() {
        let app = range(5, Some(HitRules(Box::new(Percent::new(5, 16, 30)))), Some(40));
        let odds = app.world().resource::<Asked>().0.clone().expect("percent rolls");
        assert_eq!(odds.hits, 100 - 10 - 16, "two tiles past an effective range of three, in dim light");
    }

    #[test]
    fn without_lighting_every_target_is_lit_and_no_light_line_is_given() {
        let app = range(5, Some(HitRules(Box::new(Percent::new(5, 16, 30)))), None);
        let odds = app.world().resource::<Asked>().0.clone().unwrap();
        assert_eq!(odds.hits, 90);
        assert!(odds.lines.iter().all(|l| l.label != "for dim light" && l.label != "in the dark"));
    }

    #[test]
    fn effective_defaults_to_a_third_of_range() {
        let kind = rl_rules::damage::DamageKindId::from_raw(0);
        assert_eq!(RangedAttack::new(kind, DiceRoll::flat(1), 12).effective_range(), 4);
        assert_eq!(RangedAttack::new(kind, DiceRoll::flat(1), 5).effective_range(), 1);
        assert_eq!(RangedAttack::new(kind, DiceRoll::flat(1), 12).effective_to(9).effective_range(), 9);
        assert_eq!(crate::throwing::Throwable::new(6, None).effective_range(), 2);
    }
}
```

- [ ] **Step 2: Run and see them fail**

Run: `cargo test -p rl-bevy accuracy`
Expected: compile errors, the types do not exist.

- [ ] **Step 3: Add the stats, `effective` and `Throwable::new`**

In `CombatRules` add, after `attack`:

```rust
    /// The stat an attacker's accuracy is read from, if any. With none,
    /// the hit model is told there is no accuracy and uses its own
    /// default: 100 for `Percent`.
    pub accuracy: Option<StatId>,
    /// The stat a target's evasion is read from, if any.
    pub evasion: Option<StatId>,
```

Set both `None` in `CombatRules::new`, and add builders after `attack_stat`:

```rust
    /// Reads `stat` as accuracy, for the hit model.
    pub fn accuracy_stat(mut self, stat: StatId) -> Self {
        self.accuracy = Some(stat);
        self
    }

    /// Reads `stat` as evasion, for the hit model.
    pub fn evasion_stat(mut self, stat: StatId) -> Self {
        self.evasion = Some(stat);
        self
    }
```

In `RangedAttack` add, after `range`:

```rust
    /// The furthest cell with no range penalty, for a hit model that reads
    /// one. `None` is a third of `range`, so only a weapon that cares says.
    pub effective: Option<i32>,
```

Set `effective: None` in `RangedAttack::new`, and add:

```rust
    /// No range penalty out to `cells`.
    pub const fn effective_to(mut self, cells: i32) -> Self {
        self.effective = Some(cells);
        self
    }

    /// The furthest cell with no range penalty: the one set, or a third of
    /// the range.
    pub const fn effective_range(&self) -> i32 {
        match self.effective {
            Some(cells) => cells,
            None => self.range / 3,
        }
    }
```

In `crates/rl-bevy/src/throwing.rs`, add the same `effective` field to `Throwable` (doc: `The furthest cell with no range penalty. None is a third of range.`), and:

```rust
impl Throwable {
    /// Reaching `range` and striking with `strike`, its effective range a
    /// third of that. A constructor rather than a literal, so a field only
    /// some games want is added without touching every call site.
    pub const fn new(range: i32, strike: Option<(DamageKindId, DiceRoll)>) -> Self {
        Self { range, strike, effective: None }
    }

    /// No range penalty out to `cells`.
    pub const fn effective_to(mut self, cells: i32) -> Self {
        self.effective = Some(cells);
        self
    }

    /// The furthest cell with no range penalty.
    pub const fn effective_range(&self) -> i32 {
        match self.effective {
            Some(cells) => cells,
            None => self.range / 3,
        }
    }
}
```

In `resolve_throws`, change `let Throwable { range, strike } = *throwable;` to `let Throwable { range, strike, .. } = *throwable;`. Then convert every literal in the Files list to `Throwable::new(range, strike)`.

- [ ] **Step 4: Write `crates/rl-bevy/src/accuracy.rs`**

Above the tests:

```rust
//! Whether an attack lands, as the world answers it.
//!
//! [`HitRules`] holds the game's [`HitModel`], [`Certain`] unless the game
//! inserts another, and [`Marksmanship`] is the one place the facts a model
//! reads are gathered: where the two stand, the weapon's reach, the light
//! at the target, and the stats [`CombatRules`] names. The attack resolver,
//! the throw resolver, the targeting cursor and the inspect panel all ask
//! it, so the chance a panel prints is the chance the roll uses.
//!
//! A miss is written as [`Missed`], at the moment the attack would have
//! landed, for a narrator to say.

use bevy::prelude::*;
use rl_core::geometry;
use rl_rules::StatId;
use rl_rules::accuracy::{Certain, Delivery, HitModel, Odds, Shot};

use crate::combat::{CombatRules, Loadout, RangedAttack, Reach};
use crate::components::Position;
use crate::lighting::{Lighting, band_at};
use crate::registries::Registries;
use crate::status::StatBlock;
use crate::throwing::Throwable;

/// The game's hit model. [`Certain`] by default, which rolls nothing and
/// draws nothing, so a game that never chose accuracy plays and replays
/// exactly as it did.
#[derive(Resource)]
pub struct HitRules(pub Box<dyn HitModel>);

impl Default for HitRules {
    fn default() -> Self {
        Self(Box::new(Certain))
    }
}

/// An attack that was rolled and missed.
///
/// Written where a hit would have landed: at once for a blow or an unseen
/// shot, when the flight is seen for a watched one, when the item comes
/// down for a throw. The weapon still fired, so [`Struck`](crate::combat::Struck)
/// and its `fire` moment were written; nothing else was.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Missed {
    /// Who attacked.
    pub attacker: Entity,
    /// Who it was at.
    pub target: Entity,
    /// The worn or thrown item it was made with, if any.
    pub with: Option<Entity>,
    /// How it travelled.
    pub reach: Reach,
}

/// What an attack is made with, which decides its delivery and its reach.
#[derive(Debug, Clone, Copy)]
pub enum Attempt<'a> {
    /// A blow, with whatever the attacker strikes with.
    Blow,
    /// A shot with this weapon.
    Shot(&'a RangedAttack),
    /// A throw of this.
    Throw(&'a Throwable),
}

/// The facts a hit model reads, and the one call that asks it.
///
/// Holds no [`Loadout`], which reads `Equipped`, so it can sit beside a
/// system that moves what is worn, as the throw resolver does.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Marksmanship<'w, 's> {
    rules: Res<'w, HitRules>,
    lighting: Option<Res<'w, Lighting>>,
    combat: Option<Res<'w, CombatRules>>,
    registries: Option<Res<'w, Registries>>,
    positions: Query<'w, 's, &'static Position>,
    stats: Query<'w, 's, &'static StatBlock>,
}

impl Marksmanship<'_, '_> {
    /// The odds of `attacker` landing `attempt` on `target` where the two
    /// stand now, or `None` when the model does not roll it, or either is
    /// nowhere. A throw that strikes nothing it could hurt is never rolled.
    pub fn odds(&self, attacker: Entity, target: Entity, attempt: Attempt) -> Option<Odds> {
        let (from, to) = (self.positions.get(attacker).ok()?.0, self.positions.get(target).ok()?.0);
        let (delivery, effective, range) = match attempt {
            Attempt::Blow => (Delivery::Melee, 0, 1),
            Attempt::Shot(gun) => (Delivery::Shot, gun.effective_range(), gun.range),
            Attempt::Throw(thrown) => {
                thrown.strike?;
                (Delivery::Thrown, thrown.effective_range(), thrown.range)
            }
        };
        let shot = Shot {
            delivery,
            distance: geometry::chebyshev(from, to),
            effective,
            range,
            light: band_at(self.lighting.as_deref(), to),
            accuracy: self.stat(attacker, |r| r.accuracy),
            evasion: self.stat(target, |r| r.evasion),
        };
        self.rules.0.odds(&shot)
    }

    /// The odds of what `attacker` would attack `target` with from where it
    /// stands: a blow when adjacent, a shot while its shot reaches, and
    /// nothing further, which is the rule `resolve_attacks` picks by and
    /// `forecast::Arms::at` states.
    pub fn at_distance(&self, loadout: &Loadout, attacker: Entity, target: Entity) -> Option<Odds> {
        let (from, to) = (self.positions.get(attacker).ok()?.0, self.positions.get(target).ok()?.0);
        let distance = geometry::chebyshev(from, to);
        if distance <= 1 {
            loadout.melee(attacker)?;
            return self.odds(attacker, target, Attempt::Blow);
        }
        let gun = loadout.ranged(attacker).filter(|g| distance <= g.range)?;
        self.odds(attacker, target, Attempt::Shot(&gun))
    }

    /// The value of the stat `pick` names on `who`, `None` when the game
    /// names none, so the model can tell "no stat" from "a stat of zero".
    fn stat(&self, who: Entity, pick: impl Fn(&CombatRules) -> Option<StatId>) -> Option<i32> {
        let stat = pick(self.combat.as_deref()?)?;
        let registries = self.registries.as_deref()?;
        Some(self.stats.get(who).map_or(0, |s| s.0.value(stat, &registries.stats)))
    }
}
```

Register in `CombatPlugin::build`: `.init_resource::<crate::accuracy::HitRules>().add_message::<crate::accuracy::Missed>()`.
In `crates/rl-bevy/src/lib.rs` add `pub mod accuracy;` and `pub use accuracy::{Attempt, HitRules, Marksmanship, Missed};`, and the same in the prelude.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p rl-bevy accuracy` then `cargo test --workspace`
Expected: all PASS; no other test changes, since nothing rolls yet.

- [ ] **Step 6: Docs**

`docs/guide/src/systems/combat.md`: add `crates/rl-bevy/src/accuracy.rs`, `crates/rl-bevy/src/throwing.rs` and `crates/rl-rules/src/accuracy.rs` to the manifest. In `The model`, after the `RangedAttack` sentence, add:

```markdown
`RangedAttack` and `Throwable` also carry an optional `effective` range, a third of `range` when left out, which is as far as a hit model charges nothing for distance.
`HitRules` holds the game's `HitModel`, `Certain` unless the game inserts another, and `Marksmanship` gathers what a model reads into a `Shot`: how the attack travels, the distance, the effective range and reach, the `LightBand` at the target, and the stats `accuracy_stat` and `evasion_stat` name.
```

Leave `The line` for Task 8. Bless: `python3 scripts/check-systems.py --bless combat`, and any other page the check names for `throwing.rs` after re-reading it. Run `scripts/check-systems-style.sh` on each.

- [ ] **Step 7: Run every check and commit**

```bash
git add crates examples docs/guide/src/systems
git commit -m "combat gathers what a hit model reads, and a thrown thing has an effective range"
```

---

## Task 5: Attacks miss

**Files:**
- Modify: `crates/rl-bevy/src/combat.rs` (`Arena` at 584-594, `ShotLanding` at 611-629, `resolve_attacks` at 659-721, `Arriving` and `land_shots` at 728-763, `land` at 766-772; tests)
- Modify: `crates/rl-bevy/src/throwing.rs` (`Launch` at 88-98, `ThrowLanding` at 102-109, `resolve_throws`, `ThrowReports`, `land`; tests)

**Interfaces:**
- Consumes: `HitRules`, `Missed`, `Attempt`, `Marksmanship` from Task 4.
- Produces: the behaviour. On a miss: `Struck` and the `fire` moment are written, no `DamageEvent`, no `hit` moment, one `Missed`; a thrown item rests where it would have and `ItemEvent::Thrown` reports `struck: None`.

- [ ] **Step 1: Write the failing combat tests**

In `crates/rl-bevy/src/combat.rs` `mod tests`:

```rust
    /// A model that always misses, so a test reads a miss without a seed.
    struct Never;
    impl rl_rules::accuracy::HitModel for Never {
        fn odds(&self, _: &rl_rules::accuracy::Shot) -> Option<rl_rules::Odds> {
            Some(rl_rules::Odds::new(0, 100, Vec::new()))
        }
    }

    /// Counts what one attack wrote.
    #[derive(Resource, Default, Debug)]
    struct Wrote {
        struck: usize,
        damage: usize,
        missed: Vec<crate::accuracy::Missed>,
    }

    fn count(mut wrote: ResMut<Wrote>, mut s: MessageReader<Struck>, mut d: MessageReader<DamageEvent>, mut m: MessageReader<crate::accuracy::Missed>) {
        wrote.struck += s.read().count();
        wrote.damage += d.read().count();
        wrote.missed.extend(m.read().copied());
    }

    #[test]
    fn a_miss_fires_the_weapon_and_deals_nothing() {
        for adjacent in [true, false] {
            let (mut app, start, blunt) = arena();
            app.insert_resource(crate::accuracy::HitRules(Box::new(Never))).init_resource::<Wrote>().add_systems(PostUpdate, count);
            let player = app
                .world_mut()
                .spawn((
                    (Actor, Player, Blocks, Position(start), Viewshed::new(8), Health::full(30), Faction(rl_rules::FactionId::from_raw(0))),
                    (MeleeAttack::new(blunt, DiceRoll::flat(3)), RangedAttack::new(blunt, DiceRoll::flat(3), 6)),
                ))
                .id();
            let gap = if adjacent { 1 } else { 4 };
            let target = app.world_mut().spawn((Actor, Blocks, Position(start.offset(gap, 0)), Health::full(20), Faction(rl_rules::FactionId::from_raw(1)))).id();
            app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
            app.update();
            app.update();
            let before = app.world().resource::<Turns>().now();
            app.world_mut().write_message(Intent::new(player, Attack(target)));
            app.update();
            app.update();
            assert_eq!(app.world().get::<Health>(target).unwrap().current, 20, "nothing landed");
            let wrote = app.world().resource::<Wrote>();
            assert_eq!((wrote.struck, wrote.damage), (1, 0), "the weapon fired and hurt nobody");
            let reach = if adjacent { Reach::Melee } else { Reach::Shot };
            assert_eq!(wrote.missed, vec![crate::accuracy::Missed { attacker: player, target, with: None, reach }]);
            assert!(app.world().resource::<Turns>().now() > before, "a miss is a spent turn");
        }
    }

    /// Which moments a worn weapon's attack set off.
    #[derive(Resource, Default)]
    struct Moments {
        fire: bool,
        hit: bool,
    }

    fn moments(mut seen: ResMut<Moments>, mut fired: MessageReader<crate::effects::Fired>) {
        for f in fired.read() {
            seen.fire |= f.moment == crate::effects::Moments::FIRE;
            seen.hit |= f.moment == crate::effects::Moments::HIT;
        }
    }

    #[test]
    fn a_missed_shot_with_a_worn_weapon_fires_its_fire_moment_and_not_its_hit_moment() {
        let (mut app, player, _gun, target) = gunman(3, None, None);
        app.insert_resource(crate::accuracy::HitRules(Box::new(Never))).init_resource::<Moments>().add_systems(PostUpdate, moments);
        app.world_mut().write_message(Intent::new(player, Attack(target)));
        app.update();
        app.update();
        let seen = app.world().resource::<Moments>();
        assert!(seen.fire, "the gun fired");
        assert!(!seen.hit, "and hit nothing");
        assert_eq!(app.world().get::<Health>(target).unwrap().current, 20);
    }
```

The first test's `Faction(FactionId::from_raw(..))` values follow `crate::testing::two_sides`, which registers `ours` then `theirs`; if `arena()` does not return the sides, read them with `crate::testing::two_sides`'s ids the way `a_shot_needs_a_clear_line_of_fire_and_carries_extra_strikes` does.

- [ ] **Step 2: Run and see them fail**

Run: `cargo test -p rl-bevy a_miss_fires_the_weapon a_missed_shot_with_a_worn`
Expected: FAIL, the target takes 3.

- [ ] **Step 3: Roll in `resolve_attacks`**

Add `marks: crate::accuracy::Marksmanship<'w, 's>,` and `missed: MessageWriter<'w, crate::accuracy::Missed>,` to `Arena`, and destructure them. Add a `missed: bool` field to `ShotLanding` with the doc `/// Whether it was rolled and missed: it still flies, and lands nothing.`.

In `resolve_attacks`, keep the `RangedAttack` the weapon came from so the odds can read its reach. Change the ranged arm of the weapon choice to also return the gun: build `let gun = loadout.ranged_with(actor).filter(..)` first, then map. Then, after the `fired.write(.. FIRE ..)` block and before the damage rolls:

```rust
        // Rolled after the weapon has fired and before the damage, so a
        // miss still spends what firing spends and draws no damage dice.
        // Under `Certain` there are no odds and nothing is drawn.
        let attempt = match &gun {
            Some(gun) if ranged => crate::accuracy::Attempt::Shot(gun),
            _ => crate::accuracy::Attempt::Blow,
        };
        let missed = marks.odds(actor, target, attempt).is_some_and(|odds| !odds.roll(&mut **rng));
        let hits = if missed {
            Vec::new()
        } else {
            let mut hits = vec![Hit::by(actor, kind, dice.roll_at_least(&mut **rng, 0))];
            hits.extend(loadout.strikes(actor).into_iter().map(|(kind, dice)| Hit::by(actor, kind, dice.roll_at_least(&mut **rng, 0))));
            hits
        };
```

and pass `missed` into the `ShotLanding`. Change `land` to take the `Missed` writer and write it instead of the damage and the `hit` moment:

```rust
fn land(shot: ShotLanding, at: Point, damage: &mut MessageWriter<DamageEvent>, fired: &mut MessageWriter<crate::effects::Fired>, missed: &mut MessageWriter<crate::accuracy::Missed>) {
    let (target, reach) = (shot.target, shot.reach);
    if shot.missed {
        missed.write(crate::accuracy::Missed { attacker: shot.attacker, target, with: shot.with, reach });
        return;
    }
    damage.write_batch(shot.hits.into_iter().map(|hit| DamageEvent::arriving(target, hit, reach)));
    if let Some(item) = shot.with {
        fired.write(crate::effects::Fired { on: item, moment: crate::effects::Moments::HIT, by: Some(shot.attacker), at });
    }
}
```

Add the `Missed` writer to `Arriving` for `land_shots`. A shot with a `Look` keeps its flight cue whether or not it missed. In `land_shots`, a missed shot whose worn item is gone spawns no remnant: guard the remnant block with `if !shot.missed`.

`Arena` now holds two readers of `Position` and the same resources `Loadout` reads; all are read-only, so Bevy accepts them. If Bevy reports a conflicting access (B0001), move `Marksmanship` out of `Arena` into its own parameter of `resolve_attacks`.

- [ ] **Step 4: Run the combat tests**

Run: `cargo test -p rl-bevy combat`
Expected: all PASS.

- [ ] **Step 5: Write the failing throw tests**

In `crates/rl-bevy/src/throwing.rs` `mod tests`, with the existing `Rig`. A thrown item rests on the cell of the body it struck (`flight` returns `rests: end`), so a miss rests in that same cell:

```rust
    /// Always misses.
    struct Never;
    impl rl_rules::accuracy::HitModel for Never {
        fn odds(&self, _: &rl_rules::accuracy::Shot) -> Option<rl_rules::Odds> {
            Some(rl_rules::Odds::new(0, 100, Vec::new()))
        }
    }

    /// Hits only at exactly this distance, and misses everywhere else.
    struct OnlyAt(i32);
    impl rl_rules::accuracy::HitModel for OnlyAt {
        fn odds(&self, shot: &rl_rules::accuracy::Shot) -> Option<rl_rules::Odds> {
            Some(rl_rules::Odds::new(u32::from(shot.distance == self.0), 1, Vec::new()))
        }
    }

    #[test]
    fn a_missed_throw_rests_where_a_hit_would_have_and_hurts_nobody() {
        let mut rig = Rig::new();
        rig.app.insert_resource(crate::accuracy::HitRules(Box::new(Never)));
        let target = rig.mark(3);
        let knife = rig.knives(1);
        let events = rig.throw(knife, 3);
        assert_eq!(rig.hp(target), 20, "a miss deals nothing");
        assert_eq!(rig.lying_at(rig.start.offset(3, 0)), vec![knife], "and lies where a hit would have left it");
        assert!(events.iter().any(|e| matches!(e, ItemEvent::Thrown { struck: None, .. })), "the item event says it struck nobody: {events:?}");
    }

    #[test]
    fn a_throw_that_strikes_a_body_in_between_is_rolled_against_that_body_at_its_distance() {
        let mut rig = Rig::new();
        rig.app.insert_resource(crate::accuracy::HitRules(Box::new(OnlyAt(2))));
        let near = rig.mark(2);
        let far = rig.mark(4);
        let knife = rig.knives(1);
        rig.throw(knife, 4);
        assert_eq!(rig.hp(near), 17, "rolled against the near mark at two cells, and it hit for the knife's three");
        assert_eq!(rig.hp(far), 20);
    }
```

- [ ] **Step 6: Run and see them fail**

Run: `cargo test -p rl-bevy a_missed_throw a_throw_that_strikes_a_body`
Expected: FAIL, the target is hurt.

- [ ] **Step 7: Roll in the throw resolver**

Add `marks: crate::accuracy::Marksmanship<'w, 's>` to `Launch`, and `odds: Option<rl_rules::Odds>` to `ThrowLanding` (doc: `/// The odds against whoever it struck, read as it left the hand and rolled as it lands, where the damage is rolled.`). In `resolve_throws`, after `let struck = struck.filter(..)`:

```rust
        let odds = struck.and_then(|who| marks.odds(actor, who, crate::accuracy::Attempt::Throw(throwable)));
```

Copy `*throwable` into a local before `missiles.get_mut` borrows end if the borrow checker requires it. Add `missed: MessageWriter<'w, crate::accuracy::Missed>` to `ThrowReports`. In `land`, replace the first three lines of the body with:

```rust
    let ThrowLanding { actor, item, rests, struck, strike, odds } = landing;
    commands.entity(item).insert((Position(rests), OnMap(map)));
    let target = struck.filter(|who| alive.contains(*who));
    // Rolled where the damage is rolled, as it lands, so a watched throw
    // and an unwatched one draw in the same order. A miss reports whom it
    // missed and then lands as a throw that struck nobody.
    let missed = target.is_some() && odds.is_some_and(|o| !o.roll(&mut **rng));
    if let (true, Some(target)) = (missed, target) {
        reports.missed.write(crate::accuracy::Missed { attacker: actor, target, with: Some(item), reach: crate::combat::Reach::Thrown });
    }
    let struck = if missed { None } else { target };
```

The rest of `land` is unchanged: its damage block now sees `struck: None` on a miss, and `ItemEvent::Thrown` reports `struck: None`.

- [ ] **Step 8: Run the throwing tests, then everything**

Run: `cargo test -p rl-bevy throwing` then `cargo test --workspace`
Expected: all PASS. Every existing test runs under `Certain`, so none changes; a fingerprint or replay test that changes means `Certain` drew from the stream, which is a bug to fix here, not an expectation to update.

- [ ] **Step 9: Bless and commit**

Re-read `combat.md`'s `The model` sentences about `resolve_attacks`, `Struck` and `land_shots`, and add: `A rolled attack that misses still writes `Struck` and the `fire` moment, since the weapon fired, and then writes `Missed` where it would have landed instead of any damage or `hit` moment; a thrown miss rests where a hit would have and reports that it struck nobody.` Bless `combat` and any page naming `throwing.rs`.

```bash
git add crates docs/guide/src/systems
git commit -m "a rolled blow, shot or throw can miss, and a miss still fires the weapon"
```

---

## Task 6: The log says a miss

**Files:**
- Modify: `crates/rl-ui/src/narrate.rs` (`Phrase` at 57, `Heard` at ~395, the plugin's `add_message` at ~368, `collect_narration` before the `dealt` loop at 615, the `Phrasebook` table at ~760; tests)
- Docs: `docs/guide/src/systems/narration.md`

**Interfaces:**
- Consumes: `Missed` from Task 4.
- Produces: `Phrase::YouMiss`, `Phrase::MissesYou`, `Phrase::OthersMiss`.

- [ ] **Step 1: Write the failing test**

Beside `a_shot_is_narrated_as_a_shot_and_a_blow_as_a_blow` in `crates/rl-ui/src/narrate.rs` tests, using the file's own `spoken` helper:

```rust
    /// A miss is said from where the player stands, and the droid is named
    /// the way a hit names it.
    #[test]
    fn a_miss_is_said_from_where_the_player_stands() {
        let mut stage = Stage::new(NarratorPlugin::default());
        let player = stage.player;
        let droid = stage.actor("line droid", 'd', 4, 0);
        stage.tick();
        let world = stage.app.world_mut();
        world.write_message(Missed { attacker: player, target: droid, with: None, reach: Reach::Shot });
        world.write_message(Missed { attacker: droid, target: player, with: None, reach: Reach::Shot });
        stage.tick();
        assert_eq!(spoken(&stage, &["You miss"]), vec!["You miss the line droid."]);
        assert_eq!(spoken(&stage, &["The line droid"]), vec!["The line droid misses you."]);
    }
```

- [ ] **Step 2: Run and see it fail**

Run: `cargo test -p rl-ui a_miss_is_said`
Expected: compile error, no `YouMiss`.

- [ ] **Step 3: Add the phrases and collect them**

Add to `Phrase`, after `OthersShootNothing`:

```rust
    /// You attacked someone and missed.
    YouMiss,
    /// Someone attacked you and missed.
    MissesYou,
    /// Someone attacked someone else and missed.
    OthersMiss,
```

Add `missed: MessageReader<'w, 's, Missed>,` to `Heard`, and `.add_message::<Missed>()` beside `.add_message::<DamageDealt>()` in the plugin. In `collect_narration`, immediately before `for d in heard.dealt.read()`:

```rust
    for m in heard.missed.read() {
        let phrase = if witness.is_you(m.attacker) {
            Phrase::YouMiss
        } else if witness.is_you(m.target) {
            Phrase::MissesYou
        } else {
            Phrase::OthersMiss
        };
        rows.push(say(phrase, Some(m.attacker), Some(m.target)));
    }
```

In the `Phrasebook` table, raise the array length by three and add:

```rust
            (YouMiss, "You miss {whom}.", Tones::MUTED),
            (MissesYou, "{Who} misses you.", Tones::MUTED),
            (OthersMiss, "{Who} misses {whom}.", Tones::MUTED),
```

- [ ] **Step 4: Run the narration tests**

Run: `cargo test -p rl-ui narrate`
Expected: all PASS.

- [ ] **Step 5: Docs and commit**

`docs/guide/src/systems/narration.md`: where the combat phrases are described, add `A rolled attack that missed is said as `YouMiss`, `MissesYou` or `OthersMiss`, from `Missed`, in the muted tone a blow that did nothing is said in.` Add `crates/rl-bevy/src/accuracy.rs` to its manifest if the page names `Missed`. Bless `narration`.

```bash
git add crates/rl-ui docs/guide/src/systems/narration.md
git commit -m "the log says a miss"
```

---

## Task 7: The forecast counts misses, and inspect shows the chance

**Files:**
- Modify: `crates/rl-rules/src/forecast.rs` (`Combatant` at 40-72, `expected_damage` at 132-150; test literals at 279-354)
- Modify: `crates/rl-ui/src/view/inspect.rs` (`InspectView` at 35-52, `Duelists` at 157-176, `collect_inspect` at 186-256; tests)
- Modify: `crates/rl-ui/src/panel/inspect.rs` (draw the chance under the forecast; tests)
- Docs: `docs/guide/src/systems/panels.md`, `docs/guide/src/systems/combat.md`

**Interfaces:**
- Consumes: `Marksmanship::at_distance`, `Odds` from Tasks 3-4.
- Produces: `Combatant::chance_pct: u32` (100 from both constructors), `Combatant::hitting(self, chance_pct: u32) -> Self`, `InspectView::odds: Option<Odds>`.

- [ ] **Step 1: Write the failing forecast test**

In `crates/rl-rules/src/forecast.rs` tests:

```rust
    #[test]
    fn a_blow_that_lands_half_the_time_is_expected_to_do_half_the_damage() {
        let none = Resistances::default();
        let strikes = [(DamageKindId::from_raw(0), DiceRoll::flat(6))];
        let sure = Combatant { strikes: &strikes, ..Combatant::unarmed(10, 0, 100, &none) };
        let even = Combatant { strikes: &strikes, ..Combatant::unarmed(10, 0, 100, &none) }.hitting(50);
        let target = Combatant::unarmed(30, 0, 100, &none);
        let kinds = kinds();
        let stages: [&dyn DamageStage<u32>; 1] = [&SubtractArmor];
        assert_eq!(expected_damage(&sure, &target, &kinds, &stages), 6.0);
        assert_eq!(expected_damage(&even, &target, &kinds, &stages), 3.0);
        assert_eq!(blows_to_fell(&even, &target, &kinds, &stages), Some(10), "twice the blows for half the hits");
    }
```

- [ ] **Step 2: Run and see it fail**

Run: `cargo test -p rl-rules a_blow_that_lands_half`
Expected: compile error, no `hitting`.

- [ ] **Step 3: Add `chance_pct`**

Add to `Combatant`:

```rust
    /// The chance, in whole percent, that one of its attacks lands at all:
    /// 100 unless a hit model says otherwise, so a forecast built without
    /// one reads as it always did.
    pub chance_pct: u32,
```

Set `chance_pct: 100` in `unarmed` and `armed`, add:

```rust
    /// The same combatant landing `chance_pct` percent of its attacks.
    pub fn hitting(mut self, chance_pct: u32) -> Self {
        self.chance_pct = chance_pct.min(100);
        self
    }
```

and in `expected_damage`, multiply the sum: `.sum::<f32>() * attacker.chance_pct as f32 / 100.0`. Add `chance_pct: 100` to every `Combatant { .. }` literal in the tests (lines 279, 295, 301, 337, 338, 353, 354), or rewrite each as struct update over `Combatant::unarmed`.

- [ ] **Step 4: Run the forecast tests**

Run: `cargo test -p rl-rules forecast`
Expected: PASS.

- [ ] **Step 5: Write the failing inspect tests**

In `crates/rl-ui/src/view/inspect.rs` tests, following how the existing ones open the look cursor on an actor:

```rust
    #[test]
    fn inspect_gives_the_chance_of_the_players_own_attack_from_where_it_stands() {
        let mut stage = Stage::new_with(InspectViewPlugin, |app| {
            app.insert_resource(rl_bevy::HitRules(Box::new(rl_rules::Percent::new(5, 16, 30))));
        });
        let (player, kind) = (stage.player, stage.kind);
        stage.app.world_mut().entity_mut(player).insert(RangedAttack::new(kind, rl_core::DiceRoll::flat(2), 12).effective_to(3));
        let far = stage.actor("droid", 'd', 5, 0);
        let near = stage.actor("rat", 'r', 1, 0);
        look_at(&mut stage, far);
        let odds = stage.app.world().resource::<InspectView>().odds.clone().expect("a shot reaches");
        assert_eq!(odds.percent(), 90, "two tiles past effective, in a world with no lighting");
        look_at(&mut stage, near);
        let odds = stage.app.world().resource::<InspectView>().odds.clone().expect("a blow");
        assert_eq!((odds.percent(), odds.lines.len()), (100, 0), "a blow reads no range and no light");
    }

    #[test]
    fn with_no_model_inspect_gives_no_chance() {
        let mut stage = Stage::new(InspectViewPlugin);
        let foe = stage.actor("rat", 'r', 1, 0);
        look_at(&mut stage, foe);
        assert_eq!(stage.app.world().resource::<InspectView>().odds, None);
    }
```

`look_at` is whatever the existing inspect tests use to open the cursor and put it on an entity; reuse it, or add it as a tiny helper built from theirs.

- [ ] **Step 6: Run and see them fail**

Run: `cargo test -p rl-ui inspect_gives_the_chance with_no_model_inspect`
Expected: compile error, no `odds`.

- [ ] **Step 7: Fill `odds` and scale the duel**

Add to `InspectView`:

```rust
    /// The chance the player's own attack on the subject lands, from where
    /// it stands: a blow when adjacent, a shot while it reaches. `None`
    /// when nothing reaches or the game's hit model does not roll.
    pub odds: Option<Odds>,
```

Add `marks: Marksmanship<'w, 's>,` to `Duelists`. In `collect_inspect`, reset `view.odds = None;` with the other resets; after the prop check and before building the combatants:

```rust
    let mine = duelists.marks.at_distance(&duelists.loadout, me, entity);
    let theirs = duelists.marks.at_distance(&duelists.loadout, entity, me);
    view.odds = mine.clone();
```

and chain `.hitting(mine.as_ref().map_or(100, Odds::percent))` onto `asker` and `.hitting(theirs.as_ref().map_or(100, Odds::percent))` onto `other`.

- [ ] **Step 8: Draw it**

In `crates/rl-ui/src/panel/inspect.rs`, after the lines that print the duel, print `Chance to hit: {percent}%` in the tone `odds_tone(percent)` and then each line indented two spaces as `format!("{:+} {}", line.value, line.label)`, while rows remain. Add the shared tone helper to `crates/rl-ui/src/panel/mod.rs` so the target box uses the same one:

```rust
/// The tone a chance to hit is drawn in: good from 75, worth noticing from
/// 40, bad below. One rule, so the targeting box and the inspect panel
/// agree about when a shot is a bad bet.
pub fn odds_tone(percent: u32) -> ToneId {
    match percent {
        75.. => Tones::GOOD,
        40.. => Tones::NOTICE,
        _ => Tones::BAD,
    }
}
```

Add a panel test in the style of the file's existing ones asserting the row `Chance to hit: 90%` and the row `  -10 past effective range` appear for the droid above.

- [ ] **Step 9: Run and commit**

Run: `cargo test -p rl-ui inspect` then `cargo test --workspace`.
Docs: in `panels.md`, add that inspect prints the chance to hit and its lines when the game's model rolls; in `combat.md`'s `Where it lives`, extend the `forecast.rs` sentence with `and a combatant's chance to hit scales what it is expected to deal, so a panel's word for a fight counts the misses`. Bless both.

```bash
git add crates docs/guide/src/systems
git commit -m "a forecast counts the misses, and inspect gives the chance to hit"
```

---

## Task 8: The design doc and the inventory

**Files:**
- Create: `docs/design/accuracy.md`
- Modify: `CLAUDE.md` (the layout block's `docs/design/` line lists the design docs alphabetically), `docs/README.md`, `docs/guide/src/systems/combat.md` (`The line`), `docs/design/abilities.md:457-459`, `docs/design/lighting.md:204`, `docs/TODO.md` (item 19 row at line 46 and its section at 105-106), `docs/OVERVIEW.md`, `CHANGELOG.md`, `README.md`

**Interfaces:** none; documentation only.

- [ ] **Step 1: Write `docs/design/accuracy.md`**

Follow the shape of an existing design doc such as `docs/design/stealth.md` (read it first). One sentence per line. Sections:
1. **What it is for**: every attack used to land, and why a miss is worth having (range and light mean something).
2. **The model**: `HitModel` over a `Shot`, answering `Odds { hits, out_of, lines }`; why every model reduces to that (percent, d20, opposed dice); `Certain` as the default and why it draws nothing (replays).
3. **Where the facts come from**: `Marksmanship`, stats named on `CombatRules`, `effective` on the weapon, the light band at the target.
4. **A miss**: what is and is not written, and why the weapon still fires.
5. **Rejected**: the six alternatives in section 2 of the spec, each with its reason, copied from the spec in this doc's own words.
6. **Not built**: abilities still land; minds do not weigh odds; the die's face is not reported.

- [ ] **Step 2: `The line` in `combat.md`**

Replace the sentence `Accuracy does not exist either: a blow lands unconditionally, and a to-hit roll when a game wants one is a stage that returns zero rather than a change to the resolver.` with:

```markdown
Whether an attack lands is the game's model and the engine's roll: `HitRules` holds whatever `HitModel` the game chose, `Marksmanship` hands it the same facts whoever asks, and the resolver draws once from `CombatRng`, so a panel's chance and the attack's outcome cannot disagree.
What a miss spends is the engine's call, the weapon's `fire` moment and nothing after it; what accuracy is made of, which stats, what range and light cost, whether anything is ever certain, is the model's.
```

Bless `combat`, run `scripts/check-systems-style.sh docs/guide/src/systems/combat.md` and keep `The line` intact if the budget bites.

- [ ] **Step 3: Everything else**

- `CLAUDE.md` layout block: add `accuracy` to the `docs/design/` list (alphabetical, first).
- `docs/README.md`: add `docs/design/accuracy.md` to the design doc list, following the file's format.
- `docs/design/abilities.md:457-459`: replace the bullet with `- **Abilities are not rolled.** Blows, shots and throws have a to-hit roll (docs/design/accuracy.md); an ability still lands unconditionally, and rolling one is its own slice.`
- `docs/design/lighting.md:204`: change "a ranged penalty in the dark once accuracy exists in the combat rules" to "a ranged penalty in dim and dark light, built as `Percent`'s light rule over `LightBand`".
- `docs/TODO.md`: remove item 19's table row and its section, and renumber nothing else unless the file's own convention requires it (read the top of the file).
- `docs/OVERVIEW.md`: in `rl-rules`, add `accuracy`: `Odds`, `HitModel`, `Certain`, `Percent`, `range_penalty`; in `rl-grid`, `LightBand`; in `rl-bevy`, `HitRules`, `Marksmanship`, `Missed`, and `effective` on `RangedAttack` and `Throwable`.
- `CHANGELOG.md` under `Unreleased`: `- Attacks can miss. `HitRules` holds a `HitModel`, `Certain` by default so nothing changes until a game inserts one; `Percent` is shipped. `CombatRules::accuracy_stat` and `evasion_stat` name the stats it reads.` and `- `Throwable` is built with `Throwable::new(range, strike)`; a literal no longer compiles because of the new `effective` field.` and `- `RangedAttack` and `Throwable` carry an optional `effective` range.`
- `README.md` feature list: add `To-hit rolls shaped by range and light, with the model a game's to choose.`

- [ ] **Step 4: Run every check and commit**

Run the full list, especially `scripts/check-overview.sh` (it checks the design doc is listed in `CLAUDE.md`).

```bash
git add docs CLAUDE.md CHANGELOG.md README.md
git commit -m "docs: accuracy, why it is a model and why a miss still fires the weapon"
```

---

## Task 9: The targeting view carries the range and the odds

**Files:**
- Modify: `crates/rl-ui/src/view/target.rs` (`TargetView` at 99-137, `forget` at 438-449, `Reach` at 473-487, `collect_target` at 514-600; tests)

**Interfaces:**
- Consumes: `Marksmanship`, `Attempt`, `Odds`.
- Produces: `rl_ui::view::target::Span { pub distance: i32, pub effective: i32, pub max: i32 }`, `TargetView::span: Option<Span>`, `TargetView::odds: Option<Odds>`; `TargetView::what` for a shot is now the worn gun's `Name`, empty for the user's own shot.

- [ ] **Step 1: Write the failing view tests**

In `crates/rl-ui/src/view/target.rs` tests:

```rust
    fn percent(app: &mut App) {
        app.insert_resource(rl_bevy::HitRules(Box::new(rl_rules::Percent::new(5, 16, 30))));
    }

    #[test]
    fn a_shot_carries_its_range_and_the_resolvers_odds() {
        let mut stage = Stage::new_with(TargetViewPlugin, percent);
        let (user, kind) = (stage.player, stage.kind);
        stage.app.world_mut().entity_mut(user).insert(RangedAttack::new(kind, rl_core::DiceRoll::flat(2), 12).effective_to(3));
        stage.actor("droid", 'd', 5, 0);
        stage.tick();
        stage.app.world_mut().write_message(AimFire { user });
        stage.tick();
        let view = stage.app.world().resource::<TargetView>();
        assert_eq!(view.span, Some(Span { distance: 5, effective: 3, max: 12 }));
        assert_eq!(view.odds.as_ref().map(Odds::percent), Some(90));
    }

    #[test]
    fn point_blank_is_a_blow_and_carries_the_blows_odds() {
        let mut stage = Stage::new_with(TargetViewPlugin, percent);
        let (user, kind) = (stage.player, stage.kind);
        stage.app.world_mut().entity_mut(user).insert(RangedAttack::new(kind, rl_core::DiceRoll::flat(2), 12).effective_to(0));
        stage.actor("rat", 'r', 1, 0);
        stage.tick();
        stage.app.world_mut().write_message(AimFire { user });
        stage.tick();
        let odds = stage.app.world().resource::<TargetView>().odds.clone().expect("the player has a fist");
        assert_eq!((odds.percent(), odds.lines.len()), (100, 0), "a blow reads no range and no light");
    }

    #[test]
    fn an_empty_cell_has_a_span_and_no_odds() {
        let mut stage = Stage::new_with(TargetViewPlugin, percent);
        let (user, kind) = (stage.player, stage.kind);
        stage.app.world_mut().entity_mut(user).insert(RangedAttack::new(kind, rl_core::DiceRoll::flat(2), 12));
        stage.tick();
        stage.app.world_mut().write_message(AimFire { user });
        stage.tick();
        stage.app.world_mut().resource_mut::<TargetView>().cursor = stage.at.offset(0, 4);
        stage.tick();
        let view = stage.app.world().resource::<TargetView>();
        assert_eq!(view.odds, None);
        assert_eq!(view.span.map(|s| s.distance), Some(4));
    }

    #[test]
    fn a_throw_is_rolled_against_the_body_it_strikes_first_at_that_bodys_distance() {
        let mut stage = Stage::new_with(TargetViewPlugin, percent);
        let (user, kind) = (stage.player, stage.kind);
        let knife = stage.app.world_mut().spawn((Item, Name::new("knife"), Throwable::new(9, Some((kind, rl_core::DiceRoll::flat(2)))).effective_to(1))).id();
        stage.app.world_mut().get_mut::<Inventory>(user).unwrap().add(knife);
        stage.actor("near", 'n', 2, 0);
        stage.actor("far", 'f', 6, 0);
        stage.tick();
        stage.app.world_mut().write_message(AimThrow { user, item: knife });
        stage.tick();
        stage.app.world_mut().resource_mut::<TargetView>().cursor = stage.at.offset(6, 0);
        stage.tick();
        let view = stage.app.world().resource::<TargetView>();
        assert_eq!(view.odds.as_ref().map(Odds::percent), Some(95), "one tile past effective, against the near body");
    }

    #[test]
    fn a_grenade_has_no_odds() {
        let mut stage = Stage::new_with(TargetViewPlugin, percent);
        let user = stage.player;
        let grenade = stage.app.world_mut().spawn((Item, Name::new("grenade"), Throwable::new(6, None))).id();
        stage.app.world_mut().get_mut::<Inventory>(user).unwrap().add(grenade);
        stage.actor("droid", 'd', 3, 0);
        stage.tick();
        stage.app.world_mut().write_message(AimThrow { user, item: grenade });
        stage.tick();
        assert_eq!(stage.app.world().resource::<TargetView>().odds, None);
    }
```

Match how existing tests put an item in the bag (search the file for `Inventory` near line 1008) in place of `.add(..)` if the method has another name.

- [ ] **Step 2: Run and see them fail**

Run: `cargo test -p rl-ui --lib view::target`
Expected: compile errors, no `span` or `odds`.

- [ ] **Step 3: Add the fields and fill them**

In `TargetView`, after `targets`:

```rust
    /// How far the cursor is from the user, how far the weapon reaches
    /// with no penalty, and how far it reaches at all. `None` with no aim.
    pub span: Option<Span>,
    /// The chance of landing on whoever the aim would strike, and what
    /// shaped it, from the same call the resolver rolls by. `None` when
    /// nobody would be struck, the game's model does not roll, or the aim
    /// is an ability, which is never rolled.
    pub odds: Option<Odds>,
```

and above `TargetView`:

```rust
/// Distances for the targeting box: to the cursor, the edge of no range
/// penalty, and the furthest reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// Chebyshev distance from the user to the cursor.
    pub distance: i32,
    /// The furthest cell with no range penalty.
    pub effective: i32,
    /// The furthest cell reached.
    pub max: i32,
}
```

Clear both in `forget` and at the top of `collect_target` with the other resets. Add `marks: Marksmanship<'w, 's>,` and `names: Query<'w, 's, &'static Name>,` to `Reach`.

In the `firing` branch, after `let gun = ..`, set `view.what` to the worn gun's name:

```rust
        view.what = reach.loadout.ranged_with(user).and_then(|(from, _)| from).and_then(|item| reach.names.get(item).ok()).map(|n| n.as_str().to_string()).unwrap_or_default();
        let distance = rl_core::geometry::chebyshev(from, view.cursor);
        view.span = Some(Span { distance, effective: gun.effective_range(), max: gun.range });
```

(remove the old `view.what.clear();`) and after `let legal = ..`:

```rust
        let attempt = if point_blank { Attempt::Blow } else { Attempt::Shot(&gun) };
        view.odds = target.filter(|_| legal).and_then(|who| reach.marks.odds(user, who, attempt));
```

In the throw branch, after `let thrown = ..`:

```rust
        view.span = Some(Span { distance: rl_core::geometry::chebyshev(from, view.cursor), effective: throwable.effective_range(), max: throwable.range });
        view.odds = thrown.struck.and_then(|who| reach.marks.odds(user, who, Attempt::Throw(throwable)));
```

In the ability branch, after `let def = ..`:

```rust
        let max = match def.mode {
            TargetMode::Own => 0,
            TargetMode::Adjacent => 1,
            TargetMode::Bolt { range } | TargetMode::Ball { range, .. } | TargetMode::Beam { range } => range,
            TargetMode::Cone { length } => length,
        };
        view.span = Some(Span { distance: rl_core::geometry::chebyshev(from, view.cursor), effective: max, max });
```

- [ ] **Step 4: Run the view tests**

Run: `cargo test -p rl-ui --lib view::target`
Expected: all PASS, the existing ones included.

- [ ] **Step 5: Commit**

```bash
git add crates/rl-ui
git commit -m "the targeting view carries the range and the odds the resolver rolls by"
```

---

## Task 10: The targeting box

**Files:**
- Modify: `crates/rl-ui/src/panel/target.rs` (`TargetLayout`, `TargetPanel` docs, `draw_target` from `let rect = layout.rect;` to the end of the function; module docs; tests)
- Docs: `docs/guide/src/systems/panels.md`, `docs/OVERVIEW.md`

**Interfaces:**
- Consumes: `TargetView::span`, `odds`, `what`, `targets`, `why`, `legal`; `panel::frame`, `panel::clip`, `panel::odds_tone`.
- Produces: `TargetPanel::new(rect)` draws a framed box titled `Targeting` in `rect`; a rectangle under three rows tall draws the footprint alone.

- [ ] **Step 1: Rewrite the panel tests**

Replace the two row-reading assertions in the existing tests (`stage.row(0) == "bolt at them ..."` and `"bolt at nothing - out of reach ..."` and `"dear at them - cannot pay ..."`) with box assertions, and add the new cases. Stage the box at `Rect::new(0, 0, 26, 10)` and the map below it, as `staged()` does today:

```rust
    fn staged() -> Stage {
        let mut stage = Stage::new_with((AbilitiesPlugin, rl_bevy::ThrowingPlugin, TargetPanel::new(Rect::new(0, 0, 26, 10)).hints("[tab] next")), |app| {
            app.add_engine_effects();
            abilities(app);
            app.add_plugins(rl_render::MapViewPlugin::new(Rect::new(0, 10, 40, 20)));
        });
        stage.tick();
        stage
    }

    /// The box's rows, frame stripped, trailing spaces trimmed.
    fn boxed(stage: &Stage) -> Vec<String> {
        (1..9).map(|y| stage.row(y).chars().skip(1).take(24).collect::<String>().trim_end().to_string()).collect()
    }
```

Expected rows for the existing bolt test: `boxed(&stage)[..4] == ["bolt", "Range: 2 / 6", "Target: them", ""]` (an ability has no chance row). For the out-of-reach test: `["bolt", "Range: 9 / 6", "Target: nothing", "out of reach"]`. For `dear`: `["dear", "Range: 2 / 6", "Target: them", "cannot pay"]`. The hint is in the bottom border: `stage.row(9).contains("[tab] next")`.

New tests:

```rust
    #[test]
    fn a_shot_shows_its_chance_and_every_line_behind_it() {
        let mut stage = Stage::new_with((TargetPanel::new(Rect::new(0, 0, 26, 10)),), |app| {
            app.insert_resource(rl_bevy::HitRules(Box::new(rl_rules::Percent::new(5, 16, 30))));
            app.add_plugins(rl_render::MapViewPlugin::new(Rect::new(0, 10, 40, 20)));
        });
        let (user, kind) = (stage.player, stage.kind);
        stage.app.world_mut().entity_mut(user).insert(RangedAttack::new(kind, rl_core::DiceRoll::flat(2), 12).effective_to(3));
        stage.actor("droid", 'd', 5, 0);
        stage.tick();
        stage.app.world_mut().write_message(AimFire { user });
        stage.tick();
        assert_eq!(boxed(&stage)[..5], ["fire", "Range: 5 / 12", "Target: droid", "Chance to hit: 90%", "  -10 past effective range"]);
        let chance = stage.app.world().resource::<Terminal>().get(1, 4).unwrap().fg;
        assert_eq!(chance, stage.app.world().resource::<Palette>().get(Tones::GOOD), "ninety is a good bet");
    }

    #[test]
    fn more_lines_than_rows_are_clipped_inside_the_frame() {
        let mut stage = Stage::new_with((TargetPanel::new(Rect::new(0, 0, 26, 6)),), |app| {
            app.insert_resource(rl_bevy::HitRules(Box::new(rl_rules::Percent::new(5, 16, 30))));
            app.add_plugins(rl_render::MapViewPlugin::new(Rect::new(0, 10, 40, 20)));
        });
        let (user, kind) = (stage.player, stage.kind);
        stage.app.world_mut().entity_mut(user).insert(RangedAttack::new(kind, rl_core::DiceRoll::flat(2), 12).effective_to(0));
        stage.actor("droid", 'd', 5, 0);
        stage.tick();
        stage.app.world_mut().write_message(AimFire { user });
        stage.tick();
        assert!(stage.row(5).starts_with('\u{2514}'), "the bottom border is where it belongs: {:?}", stage.row(5));
        assert!(!stage.row(6).contains("past"), "and nothing spilled below it: {:?}", stage.row(6));
    }

    #[test]
    fn with_the_cursor_down_the_box_draws_nothing() {
        let stage = staged();
        assert!(stage.rows()[..10].iter().all(|r| !r.contains("Targeting")));
    }
```

- [ ] **Step 2: Run and see them fail**

Run: `cargo test -p rl-ui --lib panel::target`
Expected: FAIL, the banner is drawn.

- [ ] **Step 3: Draw the box**

Replace everything in `draw_target` from `let rect = layout.rect;` to the end with:

```rust
    let rect = layout.rect;
    if rect.height < 3 || rect.width < 8 {
        return;
    }
    crate::panel::clear(&mut terminal, rect, &palette);
    crate::panel::frame(&mut terminal, rect, "Targeting", &layout.hints, &palette);
    let bg = palette.get(Tones::SURFACE);
    let inner = rect.inflate(-1);
    let width = inner.width as usize;
    let mut rows: Vec<(String, ToneId)> = Vec::new();
    let name = match (view.throwing, view.firing, view.what.is_empty()) {
        (Some(_), _, _) => format!("throw {}", view.what),
        (None, true, true) => "fire".to_string(),
        (None, true, false) => format!("fire {}", view.what),
        (None, false, _) => view.what.clone(),
    };
    rows.push((name, Tones::TITLE));
    if let Some(span) = view.span {
        rows.push((format!("Range: {} / {}", span.distance, span.max), Tones::TEXT));
    }
    let at = match view.targets.as_slice() {
        [] => "nothing".to_string(),
        [one] if !one.label.is_empty() => one.label.clone(),
        [_] => "one of them".to_string(),
        many => format!("{} of them", many.len()),
    };
    rows.push((format!("Target: {at}"), Tones::TEXT));
    // Red says no, and says why, where the chance would be: a refused aim
    // has no chance worth printing.
    match (view.why.first(), &view.odds) {
        (Some(reason), _) => rows.push((crate::view::ability::plain(reason).to_string(), Tones::BAD)),
        (None, Some(odds)) => {
            rows.push((format!("Chance to hit: {}%", odds.percent()), crate::panel::odds_tone(odds.percent())));
            for line in &odds.lines {
                rows.push((format!("  {:+} {}", line.value, line.label), Tones::MUTED));
            }
        }
        (None, None) => {}
    }
    for (i, (text, tone)) in rows.iter().take(inner.height as usize).enumerate() {
        terminal.print_on(inner.x, inner.y + i as i32, &clip(text, width), palette.get(*tone), bg);
    }
```

The target's name in its relation tone: if `Row` carries `relation`, print the `Target: ` prefix in `TEXT` and the name in the tone the nearby panel uses for that relation (find the helper the nearby panel calls, e.g. a `relation_tone` in `panel/nearby.rs`, and reuse it rather than writing a second). Adjust the test's colour assertions accordingly.

`plain` returns the word the banner used; check its return type and drop `.to_string()` if it is already a `String`. Update `TargetLayout`'s `rect` doc to "The box the aim is described in. Under three rows tall draws none, leaving only the footprint.", `TargetPanel::new`'s doc to match, and the module doc's paragraph about a banner to describe the box.

- [ ] **Step 4: Run the panel tests**

Run: `cargo test -p rl-ui --lib panel::target` then `cargo test -p rl-ui`
Expected: all PASS.

- [ ] **Step 5: Docs and commit**

`panels.md`: replace the description of the targeting banner with the box: what it prints in order, that a refusal takes the chance's row, that it draws only while the cursor is up, and that a game puts it at the bottom of its rail over the nearby list. `docs/OVERVIEW.md`: in the presenter table, the `TargetPanel` row becomes "the footprint on the map, and a box with the range, the target, the chance to hit and what shaped it". `CHANGELOG.md`: `- `TargetPanel::new(rect)` now draws a framed box in `rect`, not a one-row banner. Give it a box at least eight rows tall, such as the bottom of the rail, and keep hints under the box's width less four.` Bless `panels`.

```bash
git add crates/rl-ui docs CHANGELOG.md
git commit -m "the targeting cursor describes the aim in a box: range, target, chance and why"
```

---

## Task 11: The box at the bottom of every rail

**Files:**
- Modify: `examples/foundry/src/main.rs:52-67,121`, `examples/corsair/src/main.rs:77-89,162`, `examples/heist/src/main.rs:117-128,72`, `examples/delve/src/main.rs:121-133,67`

**Interfaces:**
- Consumes: the boxed `TargetPanel` from Task 10.

- [ ] **Step 1: Move the rectangle in each game**

In each `Screen::new`, after `let (nearby, hint) = panel::split_bottom(nearby, 1);`, add:

```rust
        // The targeting box sits over the bottom of the nearby list while
        // the cursor is up, where the eye already is when choosing what to
        // aim at; the rows above it stay readable.
        let (_, target) = panel::split_bottom(nearby, TARGET_ROWS);
```

with `const TARGET_ROWS: i32 = 10;` beside the game's other row constants and a one-line doc, and set `target,` in place of `target: Rect::new(map.x, map.bottom() - 1, map.width, 1),` (Corsair computes `target` as a local; replace that line). Change each game's hints to fit the border of a 26-wide rail: `"[tab] next  [esc] back"` for Corsair, Heist and Delve, and for Foundry (30 wide) keep `"[enter] fire  [tab] next"` only if it fits `RAIL - 4`; otherwise use `"[tab] next  [esc] back"`.

- [ ] **Step 2: Run each game's tests**

Run: `cargo test -p foundry -p corsair -p heist -p delve`
Expected: all PASS. A test that read the banner row off the bottom of the map fails here: point it at the box's rows instead, keeping what it asserted.

- [ ] **Step 3: Look at each one**

For each game, launch it (`cargo run -p <game>`), start a run, open the targeting cursor on a foe (Foundry and Corsair fire; Heist throws; Delve uses an ability), and screenshot the window. Check, and fix before moving on: the box's frame aligns with the rail's edges, no text crosses the frame, the nearby rows above it are whole, the highlighted row is the target, the hint fits the border, and the box vanishes with Escape. Use the `run` skill if launching needs help.

- [ ] **Step 4: Commit**

```bash
git add examples
git commit -m "every game puts the targeting box at the bottom of its rail"
```

---

## Task 12: Foundry turns accuracy on

**Files:**
- Modify: `examples/foundry/src/content.rs:88-110` (`RangedDef`), `examples/foundry/src/gear.rs:123-129,344` (`ThrowDef`), `examples/foundry/assets/items.ron` (schema comment at 33-35 and weapon rows 44-55), `examples/foundry/assets/monsters.ron` (schema comment only), the Foundry plugin where it inserts its combat resources (find `DamageStages` or `CombatRules` in `examples/foundry/src/`)
- Docs: `docs/guide/src/systems/combat.md` (`Using it`), `examples/foundry/DESIGN.md` if it describes combat

- [ ] **Step 1: Write the failing content test**

In `examples/foundry/src/content.rs` tests (or `gear.rs`, wherever items are loaded in tests):

```rust
    #[test]
    fn a_weapon_row_may_name_its_effective_range_and_one_that_does_not_gets_a_third() {
        let ranged: RangedDef = ron::from_str(r#"(range: 12, effective: 6, roll: "1d8", kind: "energy")"#).unwrap();
        assert_eq!(ranged.effective, Some(6));
        let plain: RangedDef = ron::from_str(r#"(range: 12, roll: "1d8", kind: "energy")"#).unwrap();
        assert_eq!(plain.effective, None);
    }
```

If `NameRef` cannot deserialize without a registry in a unit test, copy the approach an existing `RangedDef` test uses.

- [ ] **Step 2: Run and see it fail**

Run: `cargo test -p foundry a_weapon_row_may_name`
Expected: compile error, no field `effective`.

- [ ] **Step 3: Parse it**

`RangedDef` gains:

```rust
    /// The furthest cell with no range penalty; absent, a third of `range`.
    #[serde(default)]
    pub effective: Option<i32>,
```

and `attack()` becomes `RangedAttack { look: self.look, effective: self.effective, ..RangedAttack::new(self.kind.id(), self.roll, self.range) }`. `ThrowDef` gains the same field, and `gear.rs:344` becomes `e.insert(Throwable { effective: throw.effective, ..Throwable::new(throw.range, throw.strike.map(|(dice, kind)| (kind.id(), dice))) });`.

Update the schema comments: in `items.ron` the `ranged:` entry becomes `(range:, effective:, roll:, kind:, look:)` with `effective optional, the furthest cell with no range penalty, a third of range when absent`, and the same for `throw:`; in `monsters.ron`, the same for its `ranged:` entry.

- [ ] **Step 4: Retune the weapons**

In `examples/foundry/assets/items.ron`:

| row | change |
|---|---|
| monoblade `throw: (range: 4, ..)` | `throw: (range: 5, effective: 2, ..)` |
| hand blaster `ranged: (range: 5, ..)` | `ranged: (range: 8, effective: 3, ..)` |
| ion pistol `ranged: (range: 5, ..)` | `ranged: (range: 8, effective: 3, ..)` |
| slug pistol `ranged: (range: 6, ..)` | `ranged: (range: 9, effective: 3, ..)` |
| heavy repeater `ranged: (range: 8, ..)` | `ranged: (range: 10, effective: 4, ..)` |
| blaster carbine `ranged: (range: 9, ..)` | `ranged: (range: 12, effective: 6, ..)` |
| slug rifle `ranged: (range: 9, ..)` | `ranged: (range: 14, effective: 7, ..)` |

Grenades are unchanged; they carry no strike.

- [ ] **Step 5: Insert the model, as an anchor**

Where Foundry inserts its combat rules, add:

```rust
    // ANCHOR: accuracy
    // A shot goes wide past its weapon's effective range and in poor light:
    // five points a tile past it, sixteen for a target in dim light, thirty
    // for one seen only by the helmet in the dark. Foundry has no stats,
    // so accuracy is the model's hundred and nobody evades.
    commands.insert_resource(HitRules(Box::new(Percent::new(5, 16, 30))));
    // ANCHOR_END: accuracy
```

Use `app.insert_resource` if that is what the surrounding code does.

- [ ] **Step 6: Run Foundry's tests**

Run: `cargo test -p foundry`
Expected: tests that fire and assert damage may now fail when a shot misses. For each: if the test is about something other than accuracy, make the shot certain by putting the target within effective range in the lit band (for instance under the shoulder lamp inside four tiles), and say so in a comment; if that is not possible, insert `HitRules::default()` in that test's setup with a comment saying the test is about what it is about and not about missing. Never loosen an assertion to tolerate a miss. The droid test at `droids.rs:332` must pass unchanged.

- [ ] **Step 7: The guide's second anchor**

In `combat.md`'s `Using it`, after the tutorial snippet, add one sentence and the include:

```markdown
Turning accuracy on is one resource, and Foundry's is the whole of it.

<!-- include: ../../../../examples/foundry/src/<file>.rs:accuracy -->
```rust,no_run
```
```

Run `python3 scripts/expand-guide.py` (read `scripts/check-guide.sh` for the exact invocation) so the fenced block is filled from the anchor, add the Foundry file to `combat.md`'s manifest, and bless `combat`.

- [ ] **Step 8: Run every check and commit**

```bash
git add examples/foundry docs/guide/src/systems/combat.md
git commit -m "foundry: shots go wide at range and in poor light, and short guns reach further"
```

---

## Task 13: Play it

**Files:** whatever the play-through shows is wrong.

- [ ] **Step 1: Deck one, dim light**

`cargo run -p foundry`. On deck one, find a droid outside a wall lamp's pool. Open the targeting cursor. Screenshot the whole window. Check the box reads `fire <gun>`, `Range: n / m`, `Target: <droid>`, `Chance to hit` with `-16 for dim light` among its lines, and that the numbers match the table in the spec for the gun in hand. Check the vitals strip says `dim` when standing outside a pool and the lamp is off, and `lit` with it on.

- [ ] **Step 2: A miss**

Fire at a long shot until one misses. The log must say `You miss the <droid>.`, the gun's heat must still rise, and the droid's health must not move. Let a droid shoot at you from the dim ring and confirm a miss reads `The <droid> misses you.`.

- [ ] **Step 3: Inspect and throw**

Look at an adjacent droid with the look cursor: inspect shows the blow's chance with no lines. Throw the monoblade past a droid standing in front of another: the box's chance is against the near one.

- [ ] **Step 4: Stealth**

On a dark deck, lamp off, stand in a wall lamp's dim ring beside a droid with `lit_bonus: 6`: it should be slower to notice you than when you stand in the pool. If it is indistinguishable, report it with what you saw; do not retune `lit_bonus` without the user.

- [ ] **Step 5: Fix what looks off**

Anything misaligned, clipped, in the wrong tone, or reading oddly is fixed now, per the user's standard for pixel-perfect UI, with a test where one can pin it. Then run every check.

- [ ] **Step 6: Commit and report**

```bash
git add -A
git commit -m "foundry: what playing it with accuracy on showed"
```

Only if Step 5 changed something. Report to the user with the screenshots and the balance notes from Steps 1-4.
