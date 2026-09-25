# Accuracy, light bands and the targeting box

Status: design, agreed in conversation on 2026-09-24 against `main` at `36fcb40`.
Nothing here is built yet.

## 1. What this is for

Every blow, shot and throw lands today.
`docs/guide/src/systems/combat.md` says so on purpose, `docs/TODO.md` item 19 asks where a miss would go, and `docs/design/lighting.md` phase E waits on "a ranged penalty in the dark once accuracy exists".
Light, meanwhile, has two states in the rules and a smooth fade on the screen: a tile at intensity 16 of 255 counts as seen, and the renderer draws it at about a quarter brightness, so a player sees a monster "outside the light" that the rules call lit.
Stealth reads the same single cutoff, so stepping from a lamp's bright pool into its dim ring buys nothing, and nothing on screen says whether the player is exposed.

This work gives light three bands and a job in each, adds a to-hit roll that range and light shape, and shows the odds where the player decides.

Done when:

- melee, ranged and thrown attacks can miss, in a game that turns accuracy on, and a game that does not is byte-for-byte unchanged;
- the chance of a ranged or thrown attack falls with distance past a weapon's effective range and with less light on the target;
- the targeting cursor shows a framed box at the bottom of the rail: what is aimed, the range against the maximum, the target, the chance to hit and every line that shaped it;
- the look cursor's inspect panel shows the chance for the player's own attack on what it looks at;
- stealth's light bonus reads the lit band, and the vitals strip names the player's exposure;
- Foundry plays with all of it, checked in the running game.

## 2. What was decided

These were settled in conversation and are not reopened here.

1. **Three light bands.** `Dark` below the existing `threshold` is unseen, `Dim` from `threshold` up to a new `bright` is seen but harder to hit and easier to hide in, and `Lit` at or above `bright` is seen clearly.
2. **The roll is a model the game chooses.** Every model reduces to "a hit is `hits` chances in `out_of`", plus the labelled lines that produced it, so a percent model, a d20 model and opposed dice all fit one trait.
3. **The engine ships two models.** `Certain`, the default, rolls nothing and draws nothing. `Percent` is accuracy less evasion, less the range rule, less the light rule.
4. **Accuracy and evasion come from stats, not weapons.** `CombatRules` names an `accuracy_stat` and an `evasion_stat` the way it names `armor_stat`; with none named, accuracy is 100 and evasion is 0. A clumsy weapon is a worn item that moves the accuracy stat, which the engine already supports.
5. **No floor and no ceiling.** `Percent` clamps to 0..=100 because a probability must; a game that wants "never certain, never hopeless" writes it into its own model.
6. **Distance is relative to the weapon.** A ranged attack and a throwable carry an `effective` range, defaulting to a third of their maximum; inside it there is no penalty, and past it the model takes a fixed number of points per tile, set once by the game.
7. **A miss stops at the target.** A blow whiffs, a shot's flight plays and ends at the target, and a thrown item rests at the target's feet. The weapon still fires, so ammunition, heat and charges are spent; no damage is dealt and no `hit` moment fires. Nobody else can be hurt by a miss.
8. **Light penalizes ranged and thrown attacks only.** A blow is struck at an adjacent target, which the adjacency floor always shows.
9. **The targeting box replaces the one-line banner**, at the bottom of the right rail, drawn over the lower rows of the nearby list while the cursor is up.
10. **The melee chance is shown in the inspect panel only**, not on nearby rows.
11. **Stealth changes in the same work.** `lit_bonus` applies in the `Lit` band only, and the vitals strip shows the player's own band.
12. **Foundry is the first game to turn accuracy on.** The other four games keep `Certain` and only move the targeting box.

### The approaches weighed

- **A miss as a `DamageStage` that returns zero**, which the combat and abilities docs anticipated.
  Rejected: the weapon's `hit` moment and on-hit riders would still fire, the narrator would say "no effect" rather than "miss", and a stage cannot tell a panel the chance before the roll or list what went into it.
- **A base accuracy on each weapon.**
  Rejected: accuracy belongs to whoever holds the weapon, and a weapon that should be clumsier already has a way to say so through a stat modifier while worn.
- **A list of boxed modifiers beside a fixed percent roll**, the `DamageStages` shape.
  Rejected in favour of one model trait: a d20 game would have to fight the percent arithmetic, and two extension points for one question is one too many.
- **Distance as a flat per-tile penalty for every weapon**, or **a per-weapon slope**.
  Rejected: flat makes a long gun as bad at range as a pistol, and a slope per weapon is one more number to balance on every weapon for a crossover the base accuracy already produces.
- **Raising the seen cutoff instead of adding a band.**
  Rejected: darkness would hide monsters rather than make them hard to hit, and the fade on screen would still disagree with the rules.
- **A floor and ceiling of 5 and 95.**
  Rejected: a genre convention, not an engine need, and it would make an adjacent blow at accuracy 100 miss one time in twenty.

## 3. Light bands

- `LightBand` is `Dark`, `Dim` or `Lit`, in `crates/rl-grid/src/light.rs`, with `LightBand::of(intensity, threshold, bright)` as the one rule.
  It lives in tier 1 because `rl-rules` already depends on `rl-grid`, so the accuracy model reads the same type and the boundaries are tested with no `App`.
- In `crates/rl-bevy/src/lighting.rs`, `Lighting` gains `pub bright: u8` beside `threshold`, starting at `DEFAULT_BRIGHT`, which is 64, and `Lighting::band(p)` is `LightBand::of` over `at(p).intensity`.
- With no `LightingPlugin` there is no `Lighting`, and a caller reads every tile as `Lit`, as a world with no lighting is seen everywhere today.

Sight does not change.
`gate` and `perceives` keep their rule: seen is anything not `Dark`, plus dark sight, plus the adjacent tile.
A target seen through dark sight stands in `Dark`, which is why the light rule has a `dark` penalty as well as a `dim` one.

With `bright` at 64 and the lamps as authored, from `rl_grid::light::falloff`:

| light | lit out to | dim ring |
|---|---|---|
| Foundry shoulder lamp (200, radius 6) | 4 | 5 |
| Foundry wall lamp (170, radius 5) | 3 | 4 |
| Delve and tutorial lantern (150, radius 7) | 4 | 5 to 6 |
| Corsair lantern (200, radius 8) | 6 | 7 |
| Foundry deck one ambient (36) | none | everywhere outside a lamp's pool |
| Corsair daylight (170) | everywhere | none |

Rendering does not change.
The fade is right to look at; the band is named in words where a decision is made.

## 4. Stealth reads the lit band

`update_awareness` in `crates/rl-bevy/src/stealth.rs` passes `lit = band(subject) == Lit` to `notices`, where it passes `is_lit` today.
`notices` in `rl-rules` is unchanged, since it already takes a plain flag.

Consequences, which the Foundry and Corsair checks in play must look at:

- a subject in a lamp's dim ring, or anywhere on Foundry's deck one outside a pool, no longer gives a watcher its `lit_bonus`;
- a subject carrying a lit lamp is still `Lit`, because its own tile sits near the lamp's full intensity, so the lamp toggle stays the big stealth decision, now a readable one;
- the band is symmetric: a droid shooting at a player in a dim ring suffers the dim penalty the player would.

## 5. The roll

### 5.1 The arithmetic, in `rl-rules`

A new module, `crates/rl-rules/src/accuracy.rs`, tier 1, with no Bevy:

- `Line { label: String, value: i32 }` is one labelled contribution, in the model's own units: percentage points for `Percent`, steps of one for a d20 model.
- `Odds { hits: u32, out_of: u32, lines: Vec<Line> }` is the whole answer. `percent()` rounds `hits / out_of` for display, and `roll(&mut impl Rng) -> bool` draws once, uniformly in `0..out_of`, and hits below `hits`. `out_of` is never zero; `hits` never exceeds it.
- `Shot` is the plain facts a model reads: `reach` (`Melee`, `Shot` or `Thrown`, the same distinction `Reach` draws in `rl-bevy`, restated here so the tier holds), `distance`, `effective`, `range`, the target's `rl_grid::LightBand`, and `accuracy` and `evasion` as the stats gave them.
- `trait HitModel: Send + Sync { fn odds(&self, shot: &Shot) -> Option<Odds>; }`. `None` means "no roll": the attack lands and nothing is drawn.
- `range_penalty(distance, effective, per_tile) -> i32` is 0 at or inside `effective` and `per_tile` for every tile past it.
- `Certain` answers `None` always.
- `Percent { per_tile, dim, dark, labels }` answers `accuracy - evasion - range_penalty - light`, clamped to 0..=100, as `hits` out of 100, with one line for each term that is not zero. Its labels are the game's words, given when it is built, such as "past range" and "for dim light"; a melee `Shot` takes no range and no light term.

### 5.2 The wiring, in `rl-bevy`

- `CombatRules` gains `accuracy: Option<StatId>` and `evasion: Option<StatId>`, set by `accuracy_stat` and `evasion_stat`.
  With none named, accuracy is 100 and evasion 0.
- `RangedAttack` and `Throwable` gain `effective: Option<i32>`, set by a builder in the style of `costing`, defaulting to a third of `range` rounded down.
  Content files gain an optional `effective` beside `range`, and each RON schema's top-of-file comment says so.
- `HitRules(pub Box<dyn HitModel>)` is a resource `CombatPlugin` initializes to `Certain`, the way it initializes `DamageStages` to `SubtractArmor`.
  A game turns accuracy on by inserting its own.
- `Marksmanship`, a `SystemParam`, has `odds(attacker, target, reach) -> Option<Odds>`.
  It reads positions, the `Loadout` for the weapon's `effective` and `range`, the thrown item's `Throwable`, the named stats, and `Option<Res<Lighting>>` for the band at the target.
  The attack resolver, the throw resolver, the targeting collector, the inspect collector and a monster's attack all go through it, so the chance the box shows is the chance the roll uses.
- `rl_rules::forecast::expected_damage` is scaled by the chance where a forecast is built, so a panel's "deadly" accounts for misses.

### 5.3 A miss

- `resolve_attacks` rolls after it has chosen the weapon and written `Struck` and the `fire` moment, and before it rolls damage.
  On a miss it writes `Missed { attacker, target, with, reach }`, rolls no damage, and fires no `hit` moment.
  A shot with a `Look` still cues its flight to the target, and the airborne landing it would have waited for carries no hits.
- `resolve_throws` rolls at landing, where it already rolls damage, against whoever the flight struck, at that body's own distance.
  On a miss it writes `Missed`, deals nothing, and the item rests at the struck body's feet, where a hit would have left it.
  A throwable with no strike rolls nothing, since nothing it does depends on striking.
- The roll is drawn from `CombatRng`.
  Under `Certain` nothing is drawn, so every existing run, replay and fingerprint is unchanged.

### 5.4 Out of scope

- Abilities still land unconditionally.
- Minds roll but do not weigh the odds when choosing between tactics.
- A game that wants the die's face in the log, rather than hit or miss, is not served; `Odds` reports the chance, not the draw.

## 6. What the player sees

### 6.1 The targeting box

`TargetView` in `crates/rl-ui/src/view/target.rs` gains plain data, no colours and no strings the game did not supply:

- `distance`, `effective` and `range`, for a shot or a throw;
- `odds: Option<Odds>`, exactly as `Marksmanship::odds` returned it;
- the band at the target.

`TargetPanel` keeps its name and constructor, and its `Rect` becomes a framed box rather than a one-row banner.
It draws in `PresentSet::Overlay`, only while the cursor is up, and nothing when it is down, so the nearby list it covers is whole again.
In the screenshot's order:

```
+-Targeting-------------+
| fire carbine          |
| Range: 8 / 22         |
| Target: goblin        |
| Chance to hit: 67%    |
|   -10 past range 6    |
|   -16 for dim light   |
+-[tab] next  [esc] back+
```

- The first row is what is aimed, as the banner says it today: "fire", "throw" and the item's name, or the ability's name.
- The target's name is drawn in the tone of its relation to the aimer.
- The chance is drawn good from 75, notice from 40 and bad below, cut-offs the presenter takes as settings.
- Each line is printed as its value and its label.
- A refused aim puts the reason, in the bad tone, where the chance would be, which is what the banner's "- out of reach" says today.
- An ability aim has no chance row, since abilities land unconditionally; a throw with no strike has none either.
- The hints stay the game's, set with `.hints()`, and are drawn in the bottom border the way `panel::frame` draws every framed panel's.
- The footprint overlay on the map is unchanged.

### 6.2 Inspect

`Duel` gains `odds: Option<Odds>` for the player's attack on the subject from where the player stands, choosing the attack by the rule `Arms::at` already applies: a blow when adjacent, a shot while it reaches, none otherwise.
The inspect presenter prints "Chance to hit" and the lines under the forecast it already prints.

### 6.3 Vitals exposure

`VitalsView` gains `exposure: Option<LightBand>`, filled when `Lighting` exists, from the player's own tile.
The presenter prints it on the line that says "seen" or "hidden", as `hidden  dim`, in the good tone for dark, notice for dim and bad for lit.

### 6.4 The log

`Phrase::Missed` is collected from `Missed` and said by perspective: "You miss the goblin.", "The droid misses you.", and "The droid misses the goblin." when the player sees it.
A game rewords it in its `Phrasebook` like every other phrase.

## 7. Games

- **Foundry** inserts `HitRules(Box::new(Percent { per_tile: 5, dim: 16, dark: 30, .. }))`, with its own labels.
  It has no stats registry, so accuracy is 100 and evasion 0, and its misses come from range and light.
  Its weapons are retuned as a starting point, to be adjusted in play:

  | weapon | range today | effective / max |
  |---|---|---|
  | hand blaster, ion pistol | 5 | 3 / 8 |
  | slug pistol | 6 | 3 / 9 |
  | heavy repeater | 8 | 4 / 10 |
  | blaster carbine | 9 | 6 / 12 |
  | slug rifle | 9 | 7 / 14 |
  | thrown monoblade | 4 | 2 / 5 |

  Droid ranges are unchanged, so `droids.rs`'s test that deck two is shot from inside the lamp still holds.
  Grenades carry no strike and roll nothing.
- **Corsair, Delve, Heist and the tutorial** keep `Certain` and move `TargetPanel`'s rectangle to the bottom of their rail.
  They get the light bands and the stealth change with no change of their own.

## 8. Documentation

Each slice pays for its own, per `CLAUDE.md`:

- `docs/design/accuracy.md`, new: the model trait, why stats and not weapons, why a miss is not a damage stage, and the rejected alternatives in section 2. Listed in `CLAUDE.md`'s layout section and in `docs/README.md`.
- `docs/guide/src/systems/combat.md`: accuracy in `The model`, and `The line` loses "Accuracy does not exist" and says what the engine and the game each decide about a miss. Its manifest adds `crates/rl-rules/src/accuracy.rs` and `crates/rl-bevy/src/throwing.rs`.
- `sight.md`: the bands and `bright`.
- `stealth.md`: `lit` now means the lit band.
- `panels.md`: the box, inspect's chance and the exposure word.
- `narration.md`: `Missed`.
- Each page re-read against the code and blessed with `scripts/check-systems.py --bless`.
- `docs/OVERVIEW.md`: the new types and the changed panel.
- `CHANGELOG.md` under `Unreleased`: `TargetPanel` wants a box, `HitRules` and the two models, `Lighting::bright` and `LightBand`, and stealth's changed reading of light.
- `README.md`'s feature list: to-hit rolls shaped by range and light.
- `docs/design/abilities.md`'s "Accuracy does not exist" and `docs/design/lighting.md`'s phase E are brought up to date.
- `docs/TODO.md` item 19, "A place for a miss", is removed.

## 9. Tests

`rl-rules`, as properties over ranges where there is a property:

- `range_penalty` is zero at or inside `effective` and exactly `per_tile` a tile past it;
- `Percent` answers 0..=100 for any stats, distance and band, and answers no range or light line for a blow;
- `Odds::roll` hits at `hits / out_of` over a range of seeds, within a stated tolerance;
- `Certain` draws nothing from the generator;
- `LightBand::of` changes exactly at `threshold` and at `bright`, in `rl-grid`;
- a d20 model written in the test implements `HitModel` with its exact chance, which is the proof that the trait is not a percent trait in disguise.

`rl-bevy`:

- the targeting collector's odds equal the resolver's for the same attacker, target and world;
- a miss writes `Struck` and the `fire` moment and no `DamageEvent` and no `hit` moment;
- the existing combat fingerprints are unchanged under `Certain`, labelled as a tripwire;
- a thrown miss rests at the struck body's feet;
- a watcher's `lit_bonus` is off for a subject in a dim ring and on in a lamp's pool.

`rl-ui`, read back from the screen as the current target panel tests are:

- the box's rows for a shot, a refused aim, an ability aim and a strike-less throw;
- inspect's chance and lines;
- the exposure word beside "hidden";
- the miss phrase in both perspectives.

End to end, in the running Foundry window: aim at a droid in a dim ring on deck one and screenshot the box against the rail, fire until the log shows a miss, toggle the lamp and watch the exposure word change, then check the moved box in the other four games.

## 10. Slices

Each is one commit that passes `cargo fmt`, `cargo clippy -D warnings`, the tests and every `scripts/check-*`.

1. **Light bands.** `bright`, `LightBand`, `band`, stealth reading `Lit`, and the vitals exposure word.
2. **The roll.** `rl-rules/accuracy.rs`, `HitRules`, `Marksmanship`, misses in both resolvers, `Missed` and its phrase, the forecast scaled by the chance, and `docs/design/accuracy.md`.
3. **The box.** The targeting box, inspect's chance, and the box moved in all five games.
4. **Foundry turns it on.** `Percent`, the weapon retune, and the end-to-end pass.
