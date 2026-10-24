# Engine usability review

Written 2026-10-20 at `49c4549a`; refined 2026-10-21 at `fbbbbe5c` with every number re-measured.
Counts are from the source; timings are from two Undermere fast-tier runs on 2026-10-21.
No workspace-wide timing record exists, so engine-crate tests are counted, not timed.
The plan written from the first version, `docs/superpowers/plans/2026-10-20-engine-usability.md`, is replaced; "The plan" below lists where this review disagreed with it.
The branches that act on this review are indexed in `docs/superpowers/plans/2026-10-21-engine-usability.md`.

## The four goals

| Goal | Now | Target |
|---|---|---|
| 1. Minimal hello world | Tutorial step 1: 83 code lines, 34 engine names, 4 resources, 2 hand-written systems. Starter template: 355 code lines. | About 35 lines and 15 names: one game `Plugin`, one `PlaceRules`, a spawned player, standard keys, a test using the same plugin. |
| 2. Flexibility | Tiers 0 and 1 have no Bevy. Content loading, the place model and tile glow are reachable only through Bevy. No framework game plays in a terminal. | Everything Bevy-free in tier 1, a `TtyPlugin` for terminal and SSH play, and a toolkit example with a fight. |
| 3. Clear public modules | Module docs are good. The prelude has 722 names, games still write about 400 path imports, 26 type names are declared twice or more, and about 53 systems are public only for one test. | A core prelude sized by the tutorial, topic modules, each name declared once, nothing public only for tests. |
| 4. Ease of testing | 701 tier-1 tests need no `App`. About 550 engine `App` builds are redundant by tier-1 functions or a test could set. | Rules tested in tier 1, one `App` test per call, chance set not sampled; an estimated 30 to 55 s off the fast tier. |

## Goal 1: a minimal hello world

| | Code lines | Engine names | Resources | Path imports |
|---|---|---|---|---|
| Tutorial step 1 | 83 | 34 | 4 | 0 |
| Steps 1 to 3 | - | 75 | 9 | 5 |
| `templates/starter/src/main.rs` | 355 | 84 | 9 | 4 |
| `examples/terminal` (toolkit) | 128 | 14 | - | - |

### 1.1 Five bookkeeping steps by hand

`step01_walking.rs:81-91` inserts `TileAppearance`, `WorldMap` and `PlaceRulesRes`, spawns the player then warps it with `WarpRequest::into_place`, and sets `EngineState::Playing`.
Every tutorial step, both games and the template repeat it: 12 `PlaceRulesRes` sites, 14 warps, 14 `set(EngineState::Playing)` (115 more in tests).
`plugin.rs:853-860` warns after 120 frames when the last is forgotten.
Fix: `app.places(rules)` builds `WorldMap`; `commands.spawn_player(map, bundle)`; play starts on its own once a `WorldMap` exists and no `HoldStart` holds it.
Size: M. Plan 6.1, 6.2.

### 1.2 Step 1 hand-writes input

Step 1 writes a 23-line `player_input` (`step01_walking.rs:100-122`) though `PlayerKeysPlugin` exists; only step 10 uses it.
The template writes its own too (`main.rs:323-356`).
Fix: `PlayerKeysPlugin` from step 1 and in the template; hand-written input moves to chapter 8.
Size: S. Plan 6.5.

### 1.3 The starter template is the real first contact

The README calls it "the quickest start" (`README.md:87`); it is four times step 1.
Its test re-lists `main`'s plugins by hand (`main.rs:432-449`) instead of using `rl_engine::testing::headless` with a game plugin.
Fix: one `Starter` plugin used by `main` and the test, then 1.1 to 1.7.
Size: S after 1.1.

### 1.4 Tiles are registered twice

Each tile goes in `TileRegistry` and again in `TileAppearance`, even standard ones (`step01_walking.rs:53-59`).
Fix: `TileAppearance::standard()`, inserted by `places` when the game gives none.
Size: S.

### 1.5 `DamageStages` is written out though it is the default

The default is `[SubtractArmor]` and `CombatPlugin` inits it (`combat.rs:425-429`, `1474`), yet 13 sites write it out.
Fix: delete them.
Size: XS. Plan 2.4.

### 1.6 Factions are passed twice

`CombatRules::new(&sides)` beside `Registries { factions: sides }`: 25 sites, 15 outside tests.
Fix: `CombatRules` reads factions from `Registries`.
Size: S. Plan 6.4.

### 1.7 The log takes the turn number

`MessageLog::push`, `info`, `muted` and `notice` take `turn` (`rl-ui/src/log.rs:90-129`); about 55 sites pass it, the template passes `0`.
Fix: a `Log` system param that stamps the turn; `push_at` kept for tests.
Size: S. Plan 6.3.

### 1.8 README and guide disagree on step count

`README.md:105` says six steps, `README.md:283` nine; the binaries are numbered 1 to 6, then 8 to 10.
Fix: renumber and fix the README.
Size: XS. Plan 6.5.

### 1.9 The tutorial mixes place seeds by hand

Four tutorial steps and Foundry write `RunSeed(self.seed.0 ^ (depth as u64) << 32)` because `PlaceRules::build` gets no seed (`places.rs:207`).
The guide quotes it, so the tutorial teaches what `CLAUDE.md` forbids.
Fix: `PlaceRules::build(&self, map, seed: PlaceSeed)` (see 3.9).
Size: M. Plan 1.3.

### The target hello world

```rust
use bevy::prelude::*;
use rl_engine::prelude::*;

fn main() -> AppExit {
    App::new().add_plugins((RoguelikePlugins::new("Warren", 80, 40), Warren)).run()
}

/// The whole game, so its tests add exactly what the window does.
struct Warren;

impl Plugin for Warren {
    fn build(&self, app: &mut App) {
        app.insert_resource(Seed::from_args())
            .add_plugins(PlayerKeysPlugin::new().walk("Move", "walk").wait("Act", "wait a turn"))
            .places(Floors(TileRegistry::standard()))
            .add_systems(NewRun, |mut commands: Commands| {
                commands.spawn_player(MapId::FIRST, (Viewshed::new(9), RevealsMap, Glyph::new('@', Color::WHITE)));
            });
    }
}

struct Floors(TileRegistry);

impl PlaceRules for Floors {
    fn build(&self, _: MapId, seed: PlaceSeed) -> Result<PlaceBuild, BuildError> {
        let (wall, floor) = (self.0.expect("wall"), self.0.expect("floor"));
        let mut ctx = BaseContext::blank(80, 40, self.0.clone(), wall);
        Chain::new().then(dungeon::Rooms { floor, ..default() }).then(dungeon::RandomStart).run(&mut ctx, seed)?;
        PlaceBuild::from_context(ctx)
    }
}

#[test]
fn the_player_arrives_standing_on_floor() {
    let mut app = rl_engine::testing::headless(RunSeed(1), (), Warren);
    let me = rl_engine::testing::player(&mut app);
    assert!(app.world().resource::<WorldMap>().is_walkable(rl_engine::testing::at(&app, me)));
}
```

`MapId::FIRST`, `places`, `spawn_player`, `PlaceSeed` and `TileAppearance::standard` are proposals; the test uses today's helpers.
Making `Player` require `Actor`, `Blocks` and `Position`, as `Mind` requires its own (`minds.rs:75`), would shrink the spawn further; check the onlooker case in `components.rs:30-37` first.

### The toolkit hello world

`examples/terminal` is close already: 80 lines of game, a 48-line loop, seven plain tests.
It imports `BuildContext` only to call `outputs()` (`lib.rs:14`), which 3.6 removes.
The README's toolkit sample uses `rl_engine::` paths (`README.md:152-156`), so copying it pulls in Bevy.
It stops at walking (see 2.6).

## Goal 2: flexibility

The tier split holds: systems gather, call a tier-1 rule and apply it (`items.rs:502-512`).
It leaks in four places, and two pieces are missing.

### 2.1 Content loading needs Bevy

`rl-bevy/src/layout.rs` (643 lines) uses no Bevy; it translates `rl-rules` file types into `rl-mapgen` passes because neither crate names the other.
Eight concepts are declared in both: `Turn`, `Mirror`, `Prefer`, `Placement`, `Stairs`, `Spread`, `Thing`, `DistrictDressings`.
The Bevy-free half of `prefabs.rs` is the same seam.
Cost: a terminal game or tool that wants these places compiles Bevy, and each variant is edited in three crates.
Fix: `rl-rules` depends on `rl-mapgen`; the file types deserialize into mapgen enums, the eight twins go, and `layout.rs` moves to `rl-rules`.
The reverse dependency would make a map-only user compile 19,600 lines of rules.
Size: M. TODO 27.

### 2.2 The place model lives in `rl-bevy`

`MapId`, `Spot`, `Lock`, `PlaceBuild`, `PlaceRules`, `Ladder`, `Climb`, `StairLook` (`places.rs:50-310`) are plain data; only `MapId`'s `Component` derive ties them to Bevy, which the API review calls a mistake.
Fix: move them to tier 1 with `PlaceSeed`, so the 12 game impls migrate once.
Size: M.

### 2.3 Tile glow is read by the renderer

`rl_render::looks::light_tiles` (`looks.rs:16-21`) writes tile light, which stealth depends on.
Fix: read light in `rl-grid` or `rl-rules`; `rl-render` keeps glyphs and colors.
Size: S.

### 2.4 Every tier-2 crate compiles the window and GPU

The workspace declares one Bevy feature set (`Cargo.toml:40-60`) and all tier-2 crates take it, though `rl-bevy` uses only ECS, app, state, time and input.
Fix: each crate declares only the features it uses; this adds no cargo feature.
Helps downstream terminal builds, not this workspace.
Check `WindowFocused` in `rl-ui/src/controls.rs:387` first.
Size: S.

### 2.5 No framework game plays in a terminal or over SSH

The README says it is planned (`README.md:83`); no TODO lists it.
`rl_render::Terminal` is already the cell buffer every panel draws into, and nothing in `rl-bevy` or `rl-ui` names a window.
Missing:

1. A `TtyPlugin` writing changed cells with crossterm.
2. crossterm keys into `ButtonInput<KeyCode>`, released next frame.
3. `TtyRoguelikePlugins`: `MinimalPlugins` plus the engine's, no window plugins.
4. Resize, and the raw-mode restore guard from `examples/terminal/src/main.rs:15-22`.
5. Per-user save directories for SSH.
6. Two-cell-wide maps and glyph coverage, tested like Undermere's `typeface.rs`.

Fix: list it in `docs/TODO.md`, do 2.4, then 1 to 4.
Size: M.

### 2.6 The toolkit example stops before the rules

`examples/terminal` has no monster, blow or turn order, though the README sells `apply_hit`, `observe` and `step_cost` (`README.md:57`).
Fix: add a monster with a `TurnQueue`, a `Brain` and `apply_hit`, each as a plain tested function.
Size: S to M.

## Goal 3: well-defined, documented public modules

### 3.1 The prelude is every name, and still not enough

722 names: rl-bevy 334, rl-ui 147, rl-rules 125, rl-mapgen 37, rl-grid 29, rl-render 18, rl-save 17, rl-core 14.
Each crate's prelude is 70 to 100% of its root exports, so none is curated.
About 160 are used by no example.
Games still write about 400 path imports, led by `rl_rules::ai` tactics, `prefab`, `ability`, `faction` and `damage`.
`Rect` is left out to avoid Bevy's, so 19 example sites import it by path.
Fix: a core prelude from what steps 1 to 3 use (75 names), topic modules (`combat`, `items`, `minds`, `mapgen`, `ui`, `save`), `prelude::all` for migration; rename the engine's `Rect`.
Size: M. Plan branch 5.

### 3.2 26 type names are declared more than once

`Reach` four times, `Turn` three, and `Area`, `Asks`, `Cell`, `Change`, `DistrictDressings`, `Heard`, `Joined`, `Line`, `Look`, `Mirror`, `Offered`, `Pick`, `Piece`, `Placed`, `Placement`, `Prefer`, `Secret`, `Sides`, `Source`, `Spent`, `Spread`, `Stairs`, `Thing`, `Throw` twice each.
Eight collapse with 2.1; 18 need renaming.
The prelude silently picks one of a pair (`rl_mapgen::Secret` over `rl_bevy::secrets::Secret`).
Three of the plan's new names collide with existing ones: `Trigger`, `Statuses`, `Placement`.
Fix: the plan's `scripts/check-names.sh` in the gate, with its rename table amended.
Size: M. Plan branch 4.

### 3.3 Opaque and overloaded names

"Hold" means five things: `TurnHold`, `HoldStart`, `StatusDef::holds()`, `HeldWhileWorn`, `held_for_life`; `Charts::holds` means "contains".
Unguessable names: `Loadout`, `Facet`, `Cue`, `Moment`, `Afflict`, `Afflicted`, `Cure`, `Bestows`, `Strikes`, `Among`, `Chooser`, `Keen`, `Provenance`, `Pulse`, `Stowed`, `Life`, `Doing`.
The place names (`MapId`, `PlaceMap`, `WorldMap`, `PlaceBuild`, `PlaceRules`, `PlaceRulesRes`, `OnMap`, `WindowView`) need a one-line glossary.
Fix: the plan's renames with the collisions fixed, and a glossary page.
Size: M. Plan 4.3.

### 3.4 Public only for a test

About 53 systems are `pub` (40 in `rl-bevy`, 11 in `rl-ui`), mostly so Foundry's ambiguity test can name them.
Undermere's ambiguity test (67 lines) already avoids that by comparing apps with and without its plugin.
Fix: port Foundry's test to that form, then make engine systems `pub(crate)`.
Size: S. TODO 28.

### 3.5 Two crate roots give no module map

`rl-bevy`'s crate docs are 11 lines for 47 modules, `rl-render`'s 14 for 12; `rl-rules` and `rl-ui` list every module.
Fix: add a module map in `rl-rules`'s form.
Size: XS.

### 3.6 Generic code with one user

- `BuildContext` has one impl; remove the parameter. M. Plan 2.1.
- `Facets` interns keys only for one filter (`nearby.rs:415`); 27 sites take `ResMut<Facets>`. Use `&'static str`. S. Plan 2.3.
- `Charts` (332 lines) has no user; delete it. S. Plan 2.4.

### 3.7 Presence checks are mostly different questions

`depends_on`, `is_plugin_added`, `needs` and `reads` answer different questions; most `is_plugin_added` sites add a missing view plugin.
Bug A2 is fixed (`16304359`).
Left: three hand-written markers, and `depends_on` is a free function where the others are methods.
Fix: make `depends_on` a method; leave the markers unless a fourth appears.
Size: XS.

### 3.8 Twenty app extension traits

`AddAction`, `AddEffect`, `AddVerb` and 17 more, 19 in the prelude.
Once the prelude splits, a game globbing only the core loses their methods.
Fix: one trait per crate (`EngineApp`, `UiApp`, `SaveApp`), with the prelude split; `AddSettings` is rl-bevy's.
Size: S.

### 3.9 The random seeds

One run seed with a named stream per purpose is right: it keeps saves, replays and fingerprints stable.
The spelling and per-place derivation are not.

- Three spellings in use (`Seed::stream`, `RunSeed::rng`, `RunSeed::derive`); a fourth, `RunSeed::stream`, has no caller.
- Domains are inline byte strings: 15 in Undermere, 12 in the engine, sharing one namespace, and a typo makes a new stream.
- One Undermere floor draws from three roots: the run seed by floor, the run seed by `MapId`, and a derived place seed (`dungeon.rs:569-571`). Its `MapId` equals its floor, so only the derivation differs.
- The tutorial and Foundry add a fourth, the hand-mixed XOR of 1.9.
- Population uses three domains where three indices of one would do (`population.rs:539-616`).
- The engine's eight streams each hand-write the same `Stream` impl.

Fix: one spelling `rng(DOMAIN, index)` over a `SeedDomain` constant, `RunSeed::stream` deleted; a `seeds` module per game with a distinctness test; engine domains under `rl.`; a generic `RunRng<D>`; and a `PlaceSeed` for every per-place draw.
Re-baselines Undermere's and Foundry's fingerprints once.
Size: M. Plan branch 1.

### Features with one user

Only Undermere uses trade, records, the party and sheet panels, `Lakes`, namegen, threat scoring, `Keen` and `Reflects`: about 6,900 lines.
Foundry also uses `accrete`, layout and dressing files, `OffersPanel` and revival.
An unused plugin costs nothing at run time.
The plan's `prelude::extras` is right; cargo features or an `rl-extras` crate are not needed.

## Goal 4: ease of testing

### 4.1 Where the time goes

| Undermere fast tier | Tests | Processor s | Median s |
|---|---|---|---|
| Fingerprint | 1 | 54 to 59 | - |
| `App` | 252 | 127.4 | 0.37 |
| `App`, over seeds | 7 | 8.0 | 1.18 |
| Plain | 293 | 65.3 | 0.09 |
| Plain, over seeds | 19 | 8.5 | 0.12 |
| All | 572 | 263.0 | 0.27 |

Undermere's fast tier grew from 238 s to about 263 s in five days, against a 310 s budget.
Process start and Bevy loading cost about 0.03 s a test, 17 s across Undermere.
The workspace has 2,607 tests: 701 in tier 1 need no `App`, about 1,390 elsewhere use one.

### 4.2 Test apps re-check Undermere's content

A one-assertion town test costs 0.2 s; a content-only check costs 0.10 to 0.14 s.
`Undermere::new` reads and checks every file (`dungeon.rs:510-512`), and every test `App` does it again.
Fix: benchmark it; if checking dominates, test apps skip the check and one test does it.
Saves an estimated 10 to 25 s.
Size: S to measure, S to M to act.

### 4.3 Engine `App` tests sample rules tier 1 already proves

- Spotting: `SpotRules::chance` is unit-tested in `rl-rules/src/prop.rs`, yet five tests in `rl-bevy/src/props.rs:2841-2920` count finds over seeds: about 210 `App` builds.
- Reflection: seven tests in `ability.rs:1391-1615` build 40 apps each, 280 builds, because the chance is a hard-coded `random_bool(0.5)` (`ability.rs:565`).
- Items: 60 more for reflection on a used thing (`items.rs:1809`, `1935`).

Fix: reflection's chance becomes a rule value a test sets to always or never; the spotting tests become one test of the call.
Saves an estimated 15 to 25 s.
Size: S.

### 4.4 Rules still inside systems

`may_use`, `SpotRules::chance`, `faction::stance`, `tag::carries` and `footprint` show the pattern working.
Left:

- the reflection chance (`ability.rs:565`);
- `at_odds` in `stealth.rs:270`, a hand-written copy of `faction::stance`;
- `EQUIP_FROM_GROUND_COST` (`items.rs:365`) and `STANDING_ROOM` (`remains.rs:178`), fixed rule numbers;
- `resolve_items` (`items.rs:540-757`), 217 lines in one system, tested only through 29 `App` tests.

Fix: lift the first three (TODO 34); split `resolve_items` into gather, a tier-1 `resolve_item`, and apply.
Size: S, S, S, M.

### 4.5 State played to rather than set

Four of Undermere's slowest tests (1.9 to 2.3 s each) play 200 turns to make a worn item known.
Fix: a `familiar(app, thing)` test helper; keep one test that plays the turns.
Saves about 7 s.
Size: XS.

### 4.6 Each engine module builds its own app

`headless_app()` appears 101 times in `rl-bevy`, and modules keep their own builders (`ability.rs:1238-1260`).
This costs authors more than machine time.
Fix: leave it unless 4.3 and 4.4 leave many behind.
Size: S.

### 4.7 Test timings are not kept

`scripts/slot` writes timings into the worktree's `target/`, which is cleaned on merge.
Fix: `scripts/test-times.py` appends per-package totals to one file in the main checkout.
Size: XS.

## Tensions between the goals

- **Small prelude vs discoverability:** topic modules plus a module map and glossary; `prelude::all` for one-line imports.
- **Flexibility vs one obvious way:** toolkit and framework share one rule by design; two spellings of the same question go.
- **Short hello world vs teaching the model:** the helpers are thin, and warps and start holds move to chapter 6.
- **Fast tests vs testing the shipped game:** game behavior stays in `App` tests; rule cases move to tier 1.
- **Bevy-free content vs crate weight:** `rl-rules` on `rl-mapgen` costs a toolkit user nothing; the reverse would.
- **Lean dependencies vs no cargo features:** 2.4 only declares less.

## Ranked recommendations

By benefit per unit of work. XS is an hour, S a commit, M a branch.

| # | Recommendation | Goals | Size | After | Plan |
|---|---|---|---|---|---|
| 1 | Delete redundant `DamageStages`; fix README step count; renumber tutorial; `familiar` helper; keep test times | 1, 4 | XS | - | 2.4, 6.5; rest new |
| 2 | Play starts on its own | 1 | S | - | 6.1 |
| 3 | Reflection chance as a rule value; rate tests become call tests | 4, 3 | S | - | new |
| 4 | `PlaceSeed`, `app.places`, `spawn_player`, `TileAppearance::standard`, template as a plugin | 1, 3 | M | - | 1.3 + 6.2 merged |
| 5 | One seed spelling, seed modules, `RunRng<D>` | 3 | M | 4 | 1.1, 1.2, 1.4, 1.5 |
| 6 | `PlayerKeysPlugin` from step 1; log stamps the turn; factions once | 1 | S each | 4 | 6.3 to 6.5 |
| 7 | `rl-rules` on `rl-mapgen`; twins go; content loaders to tier 1 | 2, 3, 4 | M | - | widens 2.4; TODO 27 |
| 8 | Foundry ambiguity test in Undermere's form; systems `pub(crate)` | 3 | S | - | new; TODO 28 |
| 9 | Remove `BuildContext`, intern-free `Facets`, delete `Charts` | 3 | M | 7 | 2.1, 2.3, 2.4 |
| 10 | Measure Undermere's content load; skip re-checking in tests | 4 | S | - | new |
| 11 | Duplicate-name check, renames, one meaning of "hold" | 3 | M | 7 | branch 4, amended |
| 12 | Prelude by use, topic modules, `Rect` renamed, one app trait per crate | 3, 1 | M | 9, 11 | 3.2 + branch 5 |
| 13 | Lean Bevy features; tile glow in tier 1 | 2 | S | - | new |
| 14 | `TtyPlugin`, `TtyRoguelikePlugins`, per-user saves | 2 | M | 13 | new |
| 15 | A monster in `examples/terminal`; README toolkit sample on crate paths | 2, 1 | S to M | 7 | new |
| 16 | Place model to tier 1 | 2 | M | 4, 7 | new |
| 17 | Rule numbers and `at_odds` out of systems; split `resolve_items` | 4 | S to M | - | new; TODO 34 |
| 18 | Module maps for `rl-bevy` and `rl-render` | 3 | XS | 12 | new |

Items 1 to 3 are independent and go first.
Item 4 before 5, so `PlaceRules::build` changes once.
The plan puts hello world last; it depends on neither renames nor the prelude, so it should come forward.

## The plan

Agreed: branches 1 and 2 except task 2.2, the plan's corrections, no cargo features or `rl-extras`, the Review Focus cases.
Changes:

- **Drop 2.2 (`HitRules` as an enum).** It is a closed taxonomy, which `CLAUDE.md` forbids, and four test models use the trait. At most add `HitRules::percent(..)`.
- **3.1 is done** by `16304359`.
- **3.2 moves to branch 5**; `AddSettings` is rl-bevy's.
- **Merge 1.3 and 6.2** so the 12 `PlaceRules` impls migrate once.
- **Amend branch 4:** 26 duplicates, 8 collapse via TODO 27, and three new names collide.
- **Branch 5** starts from measured use: 34 names for step 1, 75 for steps 1 to 3, 84 for the template.
- **Branch 6 before branches 2 to 5.**
- **Counts:** `set(EngineState::Playing)` is 14 outside tests, not 49; turn-passing log calls about 55, not 38; `ResMut<Facets>` 27.
- **Missing:** goal 2 (items 7, 13 to 16) and goal 4 (items 3, 10, 17).

## Corrections to the 2026-10-20 version

- Duplicate names: 26, not 8.
- The starter template was not measured.
- `player_input` is 23 lines, not 35.
- `DamageStages` already defaults; 13 sites, not about ten.
- Turn-passing log calls: about 55, not 25; `ResMut<Facets>` sites: 27, not 41.
- Presence checks are mostly different questions, and A2 is fixed.
- `AddSettings` is declared in rl-bevy.
- `HitRules` as an enum would be a closed taxonomy.
- Undermere-only features: about 6,900 lines, not 12,000; Foundry also uses `accrete`, `dress`, `Offers` and revival; `Charts` has no user.
- The engine has no `items` seed domain; Undermere has 15 inline domains, not about 20; the tutorial's hand-mixed seeds were missed.
- The suggested order ranked by area, not benefit.

## Complex but justified

Re-checked 2026-10-21; no further review needed.

- The Bevy-free tiers, with `Brain<A>`, `Snapshot<A>` and `DamageStage<A>` generic over the actor id.
- `Effect` and `FromArgs` as two traits, for object safety.
- `HitModel` as a trait: a game's own model is the extension, and tests use it as a seam.
- Traits with many real impls: `PlaceRules`, `Pass`, `Tactic`, the `Saveable` traits.
- The turn system sets; docs should lead with the six the tutorial uses.
- The view, collector and presenter split.
- The cue and hold mechanism; only its name is in question.
- `rl_engine::testing` and `rl_bevy::testing` as public modules.
- The fingerprint's four delves (54 to 59 s): a tripwire is not sped up by changing what it plays.
- Named random streams.

## Re-measured at the end of the plan

Measured 2026-10-23 on usability branch 17, the last before 0.4.0; code lines are those outside comments and the test module.

- Core prelude: 722 names before; 77 after, with `combat` 98, `items` 59, `minds` 51, `world` 183, `mapgen` 33, `ui` 145, `save` 16 and `extras` 45 beside it.
- Tutorial step 1: 74 code lines (83 as the review counted them), 35 engine names and no path imports at the review's commit; 36 code lines, 21 engine names and no path imports after, counted the same way.
- `scripts/check-names.sh`: 26 type names declared more than once before; none after.
