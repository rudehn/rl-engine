# Engine usability review

First written 2026-10-20 at `49c4549a` from a quick survey; refined 2026-10-21 at `fbbbbe5c`, with every number below measured again.
Counts come from the source (grep, `cargo metadata --no-deps`, small scripts over the preludes and test bodies).
Timings come from the processor-time records `scripts/slot` wrote for two Undermere fast-tier runs on 2026-10-21, in the `wave-town` and `wave-palace` worktrees (572 and 574 tests).
No worktree holds a record of a workspace-wide run, so the engine crates' tests are counted here, not timed.
`docs/superpowers/plans/2026-10-20-engine-usability.md` is the plan written from the first version; where this review disagrees with it, the section "The plan" says so and why.
Nothing here is scheduled.

## The four goals

| Goal | Where it is | Where it should be |
|---|---|---|
| 1. A minimal hello world | Tutorial step 1 is 83 lines of code naming 34 engine items, four resources and two hand-written systems; the starter template is 355 lines of code. A toolkit walk is 128 lines and reads well. | About 35 lines naming about 15 items: one game `Plugin`, one `PlaceRules`, the player spawned into a place, standard keys, and a test that uses the same plugin. |
| 2. Flexibility | The tier split holds: tiers 0 and 1 have no Bevy, and the framework calls tier 1 for its rules. It leaks at the content seam: growing a place from layout, pool and dressing files, the place model and tile glow are reachable only through Bevy crates. No framework game can play in a terminal. | Everything Bevy-free lives in tier 1, a `TtyPlugin` lets any framework game play in a terminal or over SSH, and the toolkit example shows a fight, not only a walk. |
| 3. Clear, documented public modules | Module docs are good, file by file. The surface is not: the facade prelude has 722 entries yet games still write about 400 path imports, 26 type names are declared twice or more, and about 53 systems are public for one game's test. | A core prelude sized by what the tutorial uses, topic modules beside it, every type name declared once, and nothing public that only a test needs. |
| 4. Ease of testing | The functional core is in place: 701 tier-1 tests need no `App`, and the rules the README names are plain functions. The cost is in what remains: about 550 `App` builds in engine tests sample a chance over seeds that a tier-1 function already answers or a test could set, and an Undermere `App` test costs four times a plain one at the median. | Each lifted rule proved by one `App` test of its call, chance injected rather than sampled, state set up rather than played to; an estimated 30 to 55 s off the fast tier's processor time, up to a tenth. |

## Goal 1: a minimal hello world

### What a first game needs today

| | Lines | Code lines | Engine items named | Resources inserted | Path imports |
|---|---|---|---|---|---|
| `examples/tutorial` step 1 | 123 | 83 | 34 | 4 | 0 |
| Steps 1 to 3 together | 591 | - | 75 | 9 by step 3 | 5 |
| `templates/starter/src/main.rs` | 580 | 355 | 84 | 9 | 4 |
| `examples/terminal` (toolkit) | 254 | 128 | 14 | - | - |

Code lines exclude comments, blank lines and the test module.
"Engine items named" is the number of distinct facade prelude names the file uses.

### 1.1 Step 1 hands the engine five things by hand

The claim: the first chapter's `start` system does the engine's bookkeeping.
Evidence: `examples/tutorial/src/bin/step01_walking.rs:81-91` inserts `TileAppearance`, `WorldMap::new(warren.tiles.tables())` and `PlaceRulesRes(Box::new(warren))`, spawns the player at `Point::ZERO` and warps it with `WarpRequest::into_place`, and sets `EngineState::Playing`; `main` inserts `Seed(RunSeed(7))` and defines `MapId(1)` as a constant.
The same five steps are in all nine tutorial steps, Foundry, Undermere and the template: 12 `PlaceRulesRes(Box::new(..))` sites, 14 `WarpRequest::into_place` outside tests, and 14 `set(EngineState::Playing)` in the games, the tutorial and the template (115 more in tests).
`crates/rl-bevy/src/plugin.rs:853-860` warns after 120 idle frames when a game forgets the last one, which says it bites.
Why it costs the goal: `WorldMap` is always built from the same registry the rules hold, the warp always follows the spawn, and play always begins once both exist; a reader learns five names that carry no decision.
Recommendation: `app.places(rules)` inserts the rules and builds `WorldMap` from them; `commands.spawn_player(map, bundle)` spawns into the place's arrival cell; play begins on its own once a `WorldMap` exists and no `HoldStart` holds it, and the warning goes.
Size: M for the three together, one branch; the plan's tasks 6.1 and 6.2.

### 1.2 The hello world teaches a hand-written input system

The claim: step 1 writes a 23-line `player_input` (`step01_walking.rs:100-122`), with `DirectionKeys`, `Repeats`, `MyTurn` and two `MessageWriter`s, although `PlayerKeysPlugin` exists.
Evidence: `crates/rl-ui/src/player_keys.rs:38-40` says the tutorial's early steps leave the plugin out on purpose, "which teach the hand-written loop"; only step 10 adds it.
The template writes its own too (`templates/starter/src/main.rs:323-356`), through `Controls`.
Why it costs the goal: the first thing a reader copies is the part every game replaces.
Recommendation: `PlayerKeysPlugin` from step 1 and in the template; the hand-written intent moves to chapter 8, "An action of your own", which already adds a key of its own (the shove).
Size: S; plan task 6.5.

### 1.3 The starter template is the real first contact, and the old review missed it

The claim: the README calls the template "the quickest start" (`README.md:87`), and it is four times the size of step 1.
Evidence: 355 code lines; its test module rebuilds the app by hand, re-listing `main`'s plugins and systems (`templates/starter/src/main.rs:432-449`), rather than calling `rl_engine::testing::headless` with a game plugin as Undermere does (`examples/undermere/src/testing/mod.rs:40`).
It also carries the setup of 1.1, the redundant `DamageStages` of 1.5, the doubled factions of 1.6 and a log line stamped with turn `0` by hand (1.7).
Why it costs the goal: a template whose test cannot stay in step with its `main` teaches the opposite of the engine's testing rule, and its length is what a new author first measures the engine by.
Recommendation: structure it as one `struct Starter; impl Plugin for Starter`, with `main` adding `RoguelikePlugins` and `Starter`, and the test `rl_engine::testing::headless(seed, narrator, Starter)`; then apply 1.1 to 1.7.
Size: S once 1.1 lands.

### 1.4 Tile looks are a second registration of every tile

The claim: a game registers each tile twice, in `TileRegistry` and in `TileAppearance`, even for the standard tiles.
Evidence: `step01_walking.rs:53-59`; `TileRegistry::standard()` exists (`crates/rl-grid/src/tile.rs:242`) and has no matching standard appearance.
Recommendation: `TileAppearance::standard()` beside `TileRegistry::standard()`, which `places` inserts when the game gave none, so a hello world on standard tiles needs no looks; the looks file (`rl_render::looks`) stays the way a real game describes its own.
Size: S.

### 1.5 `DamageStages` is written out although it is the default

Evidence: `impl Default for DamageStages` is `[SubtractArmor]` (`crates/rl-bevy/src/combat.rs:425-429`) and `CombatPlugin` already calls `init_resource::<DamageStages>()` (`combat.rs:1474`), yet `DamageStages(vec![Box::new(SubtractArmor)])` appears at 13 sites: seven tutorial steps, the template, `rl-bevy/src/testing.rs`, `rl-bevy/src/status.rs` and three guide pages.
Recommendation: delete them; the guide's quotes follow, being includes.
Size: XS; plan task 2.4 step 2, which needs no engine change.

### 1.6 Factions are handed over twice

Evidence: `CombatRules::new(&sides)` beside `Registries { factions: sides, .. }`, 25 sites, 15 outside tests; `templates/starter/src/main.rs:212-217`.
Recommendation: `CombatRules` reads the factions from `Registries`; the API review's "a `Factions` resource out of `CombatRules`" is the same change seen from the other side.
Size: S; plan task 6.4.

### 1.7 The log takes the turn number

Evidence: `MessageLog::push`, `info`, `muted` and `notice` all take `turn: u32` (`crates/rl-ui/src/log.rs:90-129`); about 55 call sites pass it, the template passes a literal `0`.
Inside a turn `Tell` already stamps it (`crates/rl-ui/src/narrate.rs:446`); outside, the caller must find it.
Recommendation: a `Log` system param that stamps the current turn, `push_at` kept for replay and tests.
Size: S; plan task 6.3.

### 1.8 The guide and the README disagree on how many steps there are

Evidence: `README.md:105` says "six steps"; `README.md:283` says "nine runnable chapters"; there are nine binaries, numbered 1 to 6 then 8, 9 and 10, so chapter 7 runs `step08_content` and chapter 9 runs `step10_panels`.
Recommendation: renumber to `step07` to `step09` with the guide's includes in the same commit; fix the README.
Size: XS; plan task 6.5.

### 1.9 Each place's seed is mixed by hand in the tutorial

Evidence: four tutorial steps and Foundry write `RunSeed(self.seed.0 ^ (depth as u64) << 32)` (`step06_descent.rs:135`, `step08_content.rs:133`, `step09_shove.rs:138`, `step10_panels.rs:204`, `examples/foundry/src/decks.rs:262`), because `PlaceRules::build(&self, map: MapId)` is given no seed (`crates/rl-bevy/src/places.rs:207`).
Why it costs the goal: the guide quotes step 6, so the tutorial teaches the one thing `CLAUDE.md` forbids, a stream made other than through `RunSeed::derive`.
Recommendation: `PlaceRules::build(&self, map, seed: PlaceSeed)`, as the seed section below argues.
Size: M with that section; plan task 1.3.

### The target hello world

What the first chapter should be able to show once 1.1 to 1.9 land, written against the proposed names:

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

About 35 lines, two `use` lines, and the names a reader meets are the ones that carry a decision: the game's plugin, its keys, how a floor is grown, and who the player is.
`MapId::FIRST`, `places`, `spawn_player`, `PlaceSeed` and `TileAppearance::standard` are the proposals above, and `Chain::run` would take the `PlaceSeed`; the test runs today's `rl_engine::testing` helpers unchanged.
If `Player` required `Actor`, `Blocks` and `Position` as Bevy required components, as `Mind` already requires its own (`crates/rl-bevy/src/minds.rs:75`), the spawn would shrink further; that is worth checking against the onlooker case `components.rs:30-37` describes before doing.

### The toolkit hello world

`examples/terminal` is already close to the target: `generate`, `step`, `look` and `render` in 80 lines, a 48-line crossterm loop, and seven plain tests.
Two frictions: `use rl_mapgen::BuildContext` is needed only to call `ctx.outputs()` (`examples/terminal/src/lib.rs:14`), which 3.6 removes, and the README's toolkit sample imports through `rl_engine::` paths (`README.md:152-156`), so a reader who copies it into a project that depends on `rl-grid` alone gets unresolved imports, and one who adds `rl-engine` instead compiles Bevy.
What it lacks is in goal 2: it stops at walking.

## Goal 2: flexibility

The split is sound where it was drawn: `cargo metadata` confirms tiers 0 and 1 (`rl-core`, `rl-grid`, `rl-mapgen`, `rl-rules`) have no Bevy, and the framework's systems gather, call a tier-1 rule and apply it, as `may_use` calling `rl_rules::equip::may_use` shows (`crates/rl-bevy/src/items.rs:502-512`).
It leaks in four places, and two pieces are missing.

### 2.1 Growing a place from its files is reachable only through Bevy

The claim: the code that turns a layout, pool or dressing file into mapgen passes is Bevy-free and lives in a Bevy crate.
Evidence: `crates/rl-bevy/src/layout.rs` (643 lines, 9 tests) imports no Bevy at all; its own docs say "there is no plugin and no system here" (`layout.rs:1-9`).
It exists because `rl-rules` owns the files and `rl-mapgen` the passes, and neither names the other, so eight concepts are declared in both and translated arm by arm: `Turn`, `Mirror`, `Prefer`, `Placement`, `Stairs`, `Spread`, `Thing` and `DistrictDressings` (for example `rl-rules/src/dressing.rs:106`, "named twice since neither crate names the other"; `rl-bevy/src/layout.rs:215-224`).
The Bevy-free half of `crates/rl-bevy/src/prefabs.rs` is the same seam for prefabs.
Why it costs the goal: a terminal game, a level editor or a balance tool that wants Undermere's kind of place must compile Bevy, and every variant added to one of the eight is edited in three crates.
Recommendation: `rl-rules` depends on `rl-mapgen` (same tier, which `scripts/check-tiers.sh` allows; `rl-mapgen` gains `serde` derives, and `serde` is already in its tree through `rl-grid` and `rl-core`).
The file types deserialize straight into the mapgen enums, the eight twins go, and `layout.rs` with the Bevy-free part of `prefabs.rs` moves to `rl-rules` (or a `content` module in it), where its tests no longer link Bevy.
The other direction, `rl-mapgen` depending on `rl-rules`, would make a toolkit user who wants only the passes compile 19,600 lines of rules; this one keeps the lower crate small.
Size: M; this is `docs/TODO.md` item 27, and the plan's task 2.4 step 3 does one of the eight.

### 2.2 The place model lives in `rl-bevy`

Evidence: `MapId`, `Spot`, `Lock`, `PlaceBuild`, `PlaceRules`, `Ladder`, `Climb` and `StairLook` (`crates/rl-bevy/src/places.rs:50-310`) are plain data with `serde` derives; only `MapId`'s `Component` derive ties them to Bevy, and the API review already calls that derive a mistake (it inserts silently in place of `OnMap`).
Why it costs the goal: a toolkit game with more than one floor invents its own place type, and `PlaceRules::build` cannot be called without the Bevy crate, though Undermere's `Undermere::build` is already tested without an `App`.
Recommendation: move the data and the trait to `rl-mapgen` (or `rl-rules` after 2.1), keep `OnMap`, `WorldMap` and the systems in `rl-bevy`; do it together with `PlaceSeed`, which the plan already puts in `rl-core`, so the 12 game impls migrate once.
Size: M.

### 2.3 A tile's glow is read by the renderer

Evidence: `rl_render::looks::light_tiles` (`crates/rl-render/src/looks.rs:16-21`) writes a tile's `Glow` into the `TileRegistry`; its docs say "the light is gameplay, not drawing".
Why it costs the goal: lighting decides stealth, so a headless server or a terminal front end that wants tile light from the game's file must depend on `rl-render`, the Bevy sprite renderer.
Recommendation: the light half of the looks file is read in `rl-grid` (which owns `Glow`) or `rl-rules`; `rl-render` keeps the glyphs and colors.
Size: S.

### 2.4 Every tier-2 crate compiles the window and the GPU

Evidence: the workspace declares one Bevy feature set (`Cargo.toml:40-60`: `bevy_core_pipeline`, `bevy_sprite_render`, `bevy_text`, `bevy_winit`, `webgl2`, `x11`, `wayland` and more), and `rl-bevy`, `rl-ui` and `rl-save` all take it, though `rl-bevy` names nothing beyond `bevy::prelude`, `ecs`, `app`, `state`, `time` and `input` (counted over `crates/rl-bevy/src`).
Why it costs the goal: a terminal or SSH build of a framework game compiles winit and wgpu it will never open.
Recommendation: each tier-2 crate declares the Bevy features it uses; `rl-render` and the facade's `RoguelikePlugins` keep the window and render ones.
Inside this workspace Cargo unifies features, so nothing gets faster here; the benefit is for a downstream crate that depends on `rl-bevy` and `rl-ui` without `rl-render`.
This is not the cargo-feature matrix the plan rejects: no feature is added, the dependency declarations only shrink.
Size: S, but check that `rl-ui`'s use of `bevy::window::WindowFocused` (`crates/rl-ui/src/controls.rs:387`) still registers without `bevy_window`.

### 2.5 No framework game can play in a terminal or over SSH

The claim: the README says a terminal or SSH front end "is planned" (`README.md:83`), and neither `docs/TODO.md` nor `docs/OVERVIEW.md`'s "Not built yet" lists it.
Evidence of how close it is: `rl_render::Terminal` is already a back buffer of `Cell { glyph, fg, bg }` that every panel and the map view draw into (`crates/rl-render/src/terminal.rs:150-300`), and `TerminalPlugin` only adds a sprite-and-text sync over it (`terminal.rs:116-129`); nothing in `rl-bevy` or `rl-ui` names a window, a camera or a sprite.
What is missing, in order:

1. A `TtyPlugin`, in a new tier-2 crate beside `rl-render`, that inserts the same `Terminal` and in `PostUpdate` writes the cells that changed to the terminal with crossterm, in 24-bit color.
2. Input: crossterm key events written into Bevy's `ButtonInput<KeyCode>` in `PreUpdate`, each press released on the next frame, since most terminals send no release; held-key repeat (`Repeats`) then comes from the terminal's own autorepeat, which needs a test.
3. A `TtyRoguelikePlugins` group: `MinimalPlugins` with a frame-rate `ScheduleRunnerPlugin`, `StatesPlugin`, `InputPlugin`, `CorePlugin`, `FovPlugin`, `MapViewPlugin`, `UiPlugin`, the `TtyPlugin`, and none of the window, fullscreen, particles or capture plugins.
4. Terminal resize as a resize of `Terminal`, and the raw-mode restore guard `examples/terminal/src/main.rs:15-22` already shows.
5. For SSH, no network code: each session runs the binary (an `sshd` `ForceCommand`), but saves need a per-user directory, since `Saves::platform_default` writes beside the executable (`crates/rl-save/src/backend.rs:105-106`) and every SSH user would share one save.
6. A map drawn two cells across (`RoguelikePlugins::across(2)`) becomes two characters per tile, and a glyph outside the terminal font's coverage shows as a box; Undermere's typeface check (`examples/undermere/src/typeface.rs`) is the model for a test.

Recommendation: list it in `docs/TODO.md`, then build 1 to 4 with Undermere as the proof; 2.4 first, so the TTY build is lean.
Size: M.

### 2.6 The toolkit example stops before the rules

Evidence: `examples/terminal` uses `rl-core`, `rl-grid` and `rl-mapgen`; it has no monster, no blow and no turn order, although the README lists `rl_rules::damage::apply_hit`, `rl_rules::ai::awareness::observe` and `rl_core::turn::step_cost` as the toolkit's selling points (`README.md:57`).
`rl_rules::ai::Brain<A>` with `Snapshot<A>` (`crates/rl-rules/src/ai/brain.rs:277`, `snapshot.rs:151`) is usable without Bevy, but no example shows how.
Recommendation: add a monster to `examples/terminal`: a `TurnQueue`, one `Brain` deciding from a `Snapshot`, `apply_hit` on a bump, each as a plain function with a plain test.
It is the proof that the toolkit is a roguelike kit and not only a map kit, and the tier-1 rules gain a caller that is not `rl-bevy`.
Size: S to M, about 150 lines.

### What does not leak

No rule reads a `Color`; `rl_rules` content looks carry an `Rgb` of their own; the cue and hold mechanism costs nothing headless; and the code named in 2.1 to 2.3 is the only tier-2 code found with no reason to be there.

## Goal 3: well-defined, documented public modules

### 3.1 The prelude is every name, and still not enough

The claim: the facade prelude exports almost everything, yet games import by path anyway.
Evidence: `rl_engine::prelude` (`crates/rl-engine/src/lib.rs:212-224`) globs each crate's prelude: rl-bevy 334, rl-ui 147, rl-rules 125, rl-mapgen 37, rl-grid 29, rl-render 18, rl-save 17, and 14 of rl-core's 15, 722 entries in all.
Each crate's prelude is 70 to 100% of its root re-exports (rl-bevy: 334 of 403), so none is curated.
About 160 of the 722 are named by no example, the template or the tutorial, among them `Charts`, `Charting`, `AddAirborne` and 45 rl-ui names such as `EngineKey` and `RailPanels`; a dozen of those are traits whose methods games call without naming them.
Meanwhile the games write about 400 path imports (tutorial 39, Foundry 149, Undermere 216), led by `rl_rules::ai` tactics (27), `rl_rules::prefab` (17), `rl_rules::ability` (13), `rl_rules::faction` (11) and `rl_rules::damage` (11).
`Rect` is left out on purpose because Bevy's prelude has one (`lib.rs:186-190`), so every game with a panel imports `rl_engine::rl_core::Rect` by path, 19 times across the examples.
Why it costs the goal: the glob gives a reader no map, autocompletion offers 722 names, and the names a game actually needs, the tactics, `FactionDef`, `DamageKindId`, are still not in it.
Recommendation: a core prelude built from what steps 1 to 3 use (75 names today, fewer once goal 1's helpers land), topic modules beside it (`prelude::combat`, `items`, `minds`, `mapgen`, `ui`, `save`), `prelude::all` for migration, plumbing in none; tactics and faction and damage definitions join their topic module.
Rename the engine's `Rect` (for example `CellRect`) so the core prelude can carry it; the tutorial alone has 3 path imports of it.
Size: M; plan branch 5, which should take these numbers as its starting point and the two additions.

### 3.2 Twenty-six type names are declared more than once

Evidence: a scan of `pub struct|enum|type|trait` across `crates/` finds 26 names declared twice or more: `Reach` four times (`rl-bevy/src/combat.rs:501`, `rl-rules/src/arms.rs:13`, `rl-rules/src/prefab.rs:592`, `rl-ui/src/view/target.rs:609`), `Turn` three times, and `Area`, `Asks`, `Cell`, `Change`, `DistrictDressings`, `Heard`, `Joined`, `Line`, `Look`, `Mirror`, `Offered`, `Pick`, `Piece`, `Placed`, `Placement`, `Prefer`, `Secret`, `Sides`, `Source`, `Spent`, `Spread`, `Stairs`, `Thing` and `Throw` twice each.
They are two kinds: eight are one concept declared in two tier-1 crates (2.1), which collapse rather than rename; the other 18 are different things sharing a name.
The prelude picks one of a pair without saying so: it exports `rl_mapgen::Secret`, while the `Secret` a built place keeps (`rl_bevy::secrets::Secret`) is reached by path.
Recommendation: the plan's `scripts/check-names.sh`, run by the gate, with the 18 renamed and the 8 collapsed by 2.1.
Three of the plan's chosen new names collide with names that exist: `Moment` to `Trigger` meets `rl_bevy::effects::Trigger` (`effects/triggers.rs:121`), `Afflicted` to `Statuses` meets `rl_rules::status::Statuses` (`status.rs:459`), and pool's `Placed` to `Placement` meets both `Placement`s; the check would catch each, but the table should change before the work starts.
Size: M; plan branch 4.

### 3.3 Opaque and overloaded vocabulary

"Hold" means five things: `TurnHold` waits for cues (`rl-bevy/src/cue.rs:117`), `HoldStart` lets a title delay the run (`title.rs:32`), `StatusDef::holds()` makes a status skip turns (`rl-rules/src/status.rs:185`), `HeldWhileWorn` is the statuses worn gear lends (`items.rs:865`), and `held_for_life` the statuses an actor is born with (`parts.rs:801`); `Charts::holds` means "contains", and six tutorial comments call key repeat "the engine's hold".
The names a reader cannot guess from the word, each verified as public: `Loadout` (effective attacks from an actor and its gear), `Facet` (a free-text note on a UI row), `Cue` and `Cued` (a request to animate), `Moment` (what fires a trigger: use, hit, step), `Afflict`, `Afflicted` and `Cure` (apply a status, the statuses held, remove one), `Bestows` and `Strikes` (stat changes from gear, extra damage dice), `Among` and `Chooser` (which carried things an offer accepts), `Keen` (a bonus to spotting), `Provenance` (where and how deep an item was made), `Pulse` (a timer on worn gear), `Stowed` (an attack unusable now), `Life` (a dead actor kept for revival) and `Doing` (an AI's current tactic).
The place vocabulary needs a sentence: `MapId` names a place, and `PlaceMap`, `WorldMap`, `PlaceBuild`, `PlaceRules`, `PlaceRulesRes`, `OnMap` and `WindowView` hang off it.
Recommendation: the plan's renames (branch 4, task 4.3) with the three collisions above fixed, and a glossary page in the release's documentation pass.
Size: M.

### 3.4 What is public only for a test

Evidence: about 53 system functions are `pub` in the tier-2 crates (40 in `rl-bevy`, 11 in `rl-ui`), most so `examples/foundry/src/plugin/ambiguity.rs` can name them; it names 45 engine systems in 375 lines.
Undermere's ambiguity test (`examples/undermere/src/plugin/ambiguity.rs`, 67 lines) already does it the way the API review asked: it compares the game's app with and without `UndermerePlugin` (`ambiguity::added`) and names only its own systems.
Recommendation: port Foundry's test to Undermere's form, then make the engine's systems `pub(crate)`; what stays public is plugins, sets, messages, components and resources.
Size: S for Foundry, S for the visibility sweep; `docs/TODO.md` item 28, half done by Undermere's example.

### 3.5 The two largest crates' roots give no map

Evidence: every module in `rl-bevy` opens with a clear `//!` line, but `crates/rl-bevy/src/lib.rs` has 11 lines of crate docs for 47 public modules, and `rl-render` 14 for 12; `rl-rules` (89 lines) and `rl-ui` (119) list every module and what it is for.
Recommendation: a module map in `rl-bevy`'s and `rl-render`'s crate docs, in `rl-rules`'s form, grouped as the guide's system pages are.
Size: XS.

### 3.6 Generic code with one user

- `BuildContext` (`crates/rl-mapgen/src/context.rs:15`) has one implementation, `BaseContext`, and no game wraps it; every game pass is `impl Pass<BaseContext>`, and the trait shows up to a reader only as an import needed to call `outputs()`.
  Remove the parameter (plan task 2.1).
  Size: M, mechanical.
- `Facets` interns its keys (`crates/rl-ui/src/facet.rs:26-60`) only so `NearbyPanel` can filter by name (`panel/nearby.rs:415`); 27 sites take `ResMut<Facets>` to add a line of text.
  `&'static str` keys would do (plan task 2.3).
  Size: S.
- `Charts` (`crates/rl-bevy/src/charts.rs`, 332 lines, with hooks in `minds.rs`, `knowledge.rs` and `rl-save/src/engine.rs`) is used by no example; it overlaps `Knowledge`.
  Delete it (plan task 2.4).
  Size: S.
- `HitRules(Box<dyn HitModel>)` is not one of these; see "Complex but justified".

### 3.7 Presence checks: mostly four questions, not four spellings

The first version counted 62 `depends_on`, 18 `is_plugin_added`, 26 `needs`, 14 `reads` and the `*Running` checks as ways to say one thing.
They answer different questions: `depends_on` asserts a plugin another cannot work without; 14 of the 18 `is_plugin_added` sites add a plugin when the game did not, a panel its view (`rl-ui/src/panel/*.rs`) or a subsystem the effects it needs (`rl-bevy/src/effects/mod.rs:661`), and two in `finish` integrate with a plugin only if it is there (`rl-save/src/records.rs:45`, `rl-ui/src/interact.rs:58`); `needs` and `reads` are about resources the game makes.
The one real defect, bug A2, is fixed: `NoiseRunning` and `StealthRunning` read markers only their plugins insert (`16304359`, `noise.rs:223-246`).
What is left is three hand-written markers (`NoiseAdded`, `StealthAdded`, `MindsRunning`) that one generic `Running<P>` could replace, and `depends_on` being a free function where `needs` and `reads` are methods.
Recommendation: make `depends_on` a method on the app extension of 3.8; leave the markers unless a fourth appears.
Size: XS.

### 3.8 Twenty app extension traits

Evidence: 20 traits implemented for `App` (`AddAction`, `AddAirborne`, `AddChoice`, `AddControls`, `AddEffect`, `AddEngineEffects`, `AddModal`, `AddMoment`, `AddSaveable`, `AddSettings`, `AddSound`, `AddSpending`, `AddStream`, `AddTone`, `AddVerb`, `AddWork`, `KeepOnRemnant`, `Needs`, `Reads`, `ResetsOnNewRun`); 19 are in the prelude, so a game that globs it never names them.
Why it matters anyway: once the prelude is split (3.1), a game globbing only the core prelude loses every method whose trait sits in a topic module; one `EngineApp` trait in the core prelude keeps `app.add_effect` working wherever it is called.
Recommendation: merge per crate (`EngineApp` in rl-bevy, `UiApp` in rl-ui, `SaveApp` in rl-save) in the same branch as the prelude split, not before it; `AddSettings` belongs in rl-bevy's, where it is declared, not rl-ui's as the plan has it.
Size: S.

### 3.9 The random seeds

The design is right at its core: one `RunSeed` per run and an independent named stream per purpose, so a draw added in one system never shifts another's, which is what keeps saves, replays and fingerprints stable.
What confuses is the spelling and where a place's draws come from.

- **Three spellings in use and a fourth with no caller.** `Seed::stream(b"name", i) -> StdRng` (`crates/rl-bevy/src/seed.rs:59`), `RunSeed::rng(SeedDomain::new(b"name"), i) -> StdRng` and `RunSeed::derive(domain, i) -> u64` (`crates/rl-core/src/seed.rs:53-64`); `RunSeed::stream(domain) -> u64` (`seed.rs:58`) is called nowhere outside its own file.
  Undermere uses the first two, 7 times each, and `derive` once.
- **Domains are byte strings typed inline.** Undermere has 15 draw sites, one domain each, all `undermere.`-prefixed but `lore` (`examples/undermere/src/lore.rs:226`); the engine has 12 (`ability`, `ability.reflect`, `combat`, `fire`, `gas spill`, `gas swirl`, `loot`, `minds`, `props`, `restart`, `status.stumble`, `stealth`).
  A typo quietly makes a new stream, nothing lists them, and a game's names share one namespace with the engine's.
- **A place's randomness has three roots.** In Undermere, `recipe`, `biome`, `population` and `finds` derive from the run seed by floor number, `vaults.fixtures`, `vaults.marks` and `writings` by `MapId`, and the chain itself from a place seed derived under `undermere.place` (`examples/undermere/src/dungeon.rs:569-571`).
  Since Undermere's `MapId` is its floor number (`dungeon.rs:120-122`), the index is the same integer; the inconsistency is the derivation, not the key.
  The tutorial and Foundry add a fourth way, the hand-mixed XOR of 1.9.
- **Some streams are split finer than they need to be.** One floor's population draws from `population`, `population.bands` and `population.plan` (`population.rs:539-616`); indices 0, 1 and 2 of one domain would keep them independent.
- **The engine's eight streams hand-write one `Stream` impl each**: `CombatRng`, `MindRng`, `LootRng`, `StealthRng`, `PropRng`, `EffectRng`, `ReflectRng` and `StumbleRng` (`combat.rs:438`, `minds.rs:760`, `loot.rs:170`, `stealth.rs:279`, `props.rs:664`, `effects/mod.rs:59`, `ability.rs:409`, `status.rs:515`).

Recommendations, as the plan's branch 1 has them: one spelling, `rng(DOMAIN, index)` over a `SeedDomain` constant, with `RunSeed::stream` deleted; each game's domains as constants in one `seeds` module with a distinctness test, and the engine's under an `rl.` prefix no game domain may take; a generic `RunRng<D>` for the eight streams; and a `PlaceSeed` handed to `PlaceRules::build`, from which every per-place draw comes.
It re-baselines Undermere's and Foundry's fingerprints once.
Size: M.

### Features with one user

Undermere is the only user of trade (`rl-bevy/src/trade.rs` 774, `rl-rules/src/trade.rs` 172, the panel 556 and view 288), records (433, 210 and a 465-line panel), the party and sheet panels and views (1,305), `Lakes` (1,364), namegen and names (1,135), threat scoring, `Keen` and `Reflects`, about 6,900 lines.
Foundry also uses `accrete`, the layout and dressing files, `OffersPanel` and revival, so those are shared.
Each is general, and an unused plugin costs nothing at run time.
The plan's answer, out of the core prelude and into `prelude::extras` rather than behind cargo features or in an `rl-extras` crate, is right for goal 3; compile time is the only cost left, and it is unmeasured.

## Goal 4: ease of testing

### 4.1 Where the time goes

| Undermere fast tier, 2026-10-21 | Tests | Processor s | Median s |
|---|---|---|---|
| The fingerprint (four scripted delves) | 1 | 53.7 to 58.5 | - |
| Build an `App` | 252 | 127.4 | 0.37 |
| Build an `App`, over seeds | 7 | 8.0 | 1.18 |
| Plain | 293 | 65.3 | 0.09 |
| Plain, over seeds | 19 | 8.5 | 0.12 |
| All | 572 | 263.0 | 0.27 |

A test is counted as building an `App` when its body calls `headless`, an `arena`, `continued` or `App::new`; the two runs agree within about 2%.
Undermere's fast tier was 238 s when its budget was set on 2026-10-16 and is 258 to 263 s five days later, against a budget of 310 s.
The cheapest Undermere test costs 0.03 s, which bounds the floor of starting a process and loading the Bevy dylib; at 572 tests that floor alone is about 17 s.
The workspace has 2,607 `#[test]`s: 701 in the tier-1 crates need no `App`, and about 1,390 elsewhere touch one (455 of rl-bevy's 555, 347 of rl-ui's 431).

### 4.2 An `App` test's setup is a game's content, read and checked again

The claim: most of an Undermere `App` test's floor is the game's setup, not the property.
Evidence: a test that builds the town and asserts one thing costs about 0.2 s (`tests::new_game_from_the_title_puts_the_adventurer_in_mereholt`, 0.21 s); a test that only reads and checks content costs 0.10 to 0.14 s (`prefabs::tests::every_piece_loads_against_the_tiles_the_props_and_the_bestiary`, `content::tests::nothing_in_the_content_is_orphaned`); `Undermere::new` reads and checks every file, "which is most of what `new` costs" (`examples/undermere/src/dungeon.rs:510-512`), and `UndermerePlugin` does it again for every `App`.
nextest runs each test in its own process, so a cache kept for the process saves nothing.
Recommendation: first measure `Undermere::new` and the plugin's content load with a bench; if checking is most of it, the test build reads content without re-checking it and one test checks it, which is where "the loader refuses a file with every problem at once" is proved.
Estimate: up to about 0.1 s off each of 259 `App` tests, 10 to 25 s of processor time.
Size: S to measure, S to M to act on.

### 4.3 Engine `App` tests sample rules that tier 1 already proves

The claim: about 550 `App` builds in rl-bevy's tests exist to see a chance come up; one chance is already a tier-1 function with unit tests of its own, and the other is a rule number a test cannot set.
Evidence:

- Spotting: `SpotRules::chance` is a plain function in `crates/rl-rules/src/prop.rs:466`, unit-tested at `prop.rs:707-715`; `crates/rl-bevy/src/props.rs:2841-2920` still checks its halving, keenness and stillness by counting finds over 60, 20 and 10 seeds, each seed its own `App`, about 210 builds across five tests.
- Reflection: seven tests in `crates/rl-bevy/src/ability.rs:1391-1615` each build 40 `App`s so both outcomes of a reflection are seen, 280 builds, because the chance is a `random_bool(0.5)` inside a system param (`ability.rs:565`), a rule number in the engine that the API review's section F already lists.
- Items: `items.rs:1809` and `items.rs:1935`, 60 more for the same reflection on a used thing.

Why it costs the goal: this is what `CLAUDE.md` calls a rate tested by playing; the rule's cases belong to its unit test, and the `App` test should prove only that the system calls it.
Recommendation: reflection's chance becomes a rule value (`ReflectRules { chance }` or a field of an existing rules resource), so a test sets it to never or always and builds two apps; the spotting rate tests become one `App` test of the call (a certainty in sight found, one out of sight never) beside the unit test that exists.
Estimate: no workspace timing exists to price an rl-bevy `App`; at an assumed 0.03 to 0.05 s each, removing about 500 builds saves 15 to 25 s of processor time.
Size: S.

### 4.4 Rules still inside systems

The functional-core refactor lifted most of them, and the pattern is followed: `may_use`, `SpotRules::chance`, `rl_rules::faction::stance`, `rl_rules::tag::carries` (`b3d07839`) and `rl_grid::footprint` (`ed0087ca`) are each one tier-1 rule a system calls.
What is left and forces `App` tests:

- the reflection chance above (`ability.rs:565`);
- `stealth.rs:270`, `at_odds`, a hand-written faction match beside `rl_rules::faction::stance`, which the API review counts among about 15;
- `EQUIP_FROM_GROUND_COST` (`items.rs:365`) and `STANDING_ROOM` (`remains.rs:178`), rule numbers the engine fixes, so a game cannot change them and a test reaches them only through an `App`;
- `resolve_items` (`items.rs:540-757`), 217 lines in one system resolving pickups, drops, equips and uses, whose refusals (`Stuck`, out of reach, not carried) are tested only through `items.rs`'s 29 `App` tests.

Recommendation: the first three are S each and belong with `docs/TODO.md` item 34; `resolve_items` splits into gather, a tier-1 `resolve_item(intent, facts) -> Outcome`, and apply, which turns most of those 29 into unit tests.
Size: S, S, S, M.

### 4.5 State played to rather than set up

Evidence: four of Undermere's five slowest non-fingerprint tests (1.9 to 2.3 s each) play `FAMILIAR_AFTER`, 200 turns (`examples/undermere/src/lore.rs:61`), to make a worn thing known: `enchant.rs:302`, `armory.rs:1647`, `lore::tests::armor_found_is_called_unidentified_until_it_has_been_worn_two_hundred_turns` and `seeming::tests::a_worn_thing_not_yet_known_...`.
Recommendation: a `familiar(app, thing)` helper in Undermere's `testing` that sets the worn count, and one test that plays the 200 turns.
Estimate: about 7 s, 3% of Undermere's fast tier.
Size: XS.

### 4.6 Each engine module assembles its own app

Evidence: `headless_app()` appears 101 times in `crates/rl-bevy/src`, and modules keep their own builders that add plugins, content and registries by hand (`ability.rs:1238-1260` is 22 lines).
This is setup cost for the author more than for the machine: a new rl-bevy feature starts by copying one.
Recommendation: leave it unless 4.3 and 4.4 leave many behind; an engine-side builder taking a plugin tuple and a content string is the shape if it is wanted.
Size: S.

### 4.7 The timings a review needs are not kept

Evidence: `scripts/slot` writes `target/nextest/cpu/*.tsv` in whichever worktree ran the gate, and a worktree's `target/` is cleaned when its branch merges, so the main checkout holds no record and the only ones on the machine are two Undermere runs.
Recommendation: `scripts/test-times.py` appends each run's per-package totals to one file in the main checkout (or `docs/`), so the budget's history and the engine crates' costs survive the worktree.
Size: XS.

## Tensions between the goals

- **A small prelude against discoverability.** A small glob hides names; a large one hides structure.
  Resolved by topic modules: the core prelude is what a first game needs, each guide chapter adds the module its subject is in, and `prelude::all` stays for a game that wants one line.
  Discoverability comes from crate docs with a module map (3.5) and a glossary, not from the glob.
- **Flexibility against one obvious way.** The toolkit and the framework are two ways to run one rule, by design, and the rule is written once.
  Inside the framework, two ways survive only where they answer different questions: `Tell` inside a turn and `MessageLog` outside it; `depends_on` for a plugin and `needs` for a resource.
  Two spellings of one question go: the seed spellings, the twin types.
- **A short hello world against teaching the model.** Hiding `WarpRequest` and `EngineState` teaches less of the turn loop in chapter 1.
  The helpers are thin and named for what they do, and chapter 6, "Two floors", is where warps and holding the start belong.
- **Fast tests against testing the game that ships.** Undermere's `headless` adds the real `UndermerePlugin` so a test plays the shipped game.
  Kept for what is about the game; a rule's cases move to tier 1, and only the call is tested through the app.
- **Bevy-free content against crate weight.** `rl-rules` depending on `rl-mapgen` (2.1) costs a toolkit user nothing, since `serde` is already in both trees; the other direction would cost a map-only user the rules crate.
- **Lean dependencies against the plan's "no cargo features".** 2.4 adds no feature and no CI matrix; it only declares less, so the plan's reasoning stands.

## Ranked recommendations

Ordered by benefit to the four goals per unit of work.
XS is an hour, S a commit, M a branch.

| # | Recommendation | Goals | Size | Depends on | Plan |
|---|---|---|---|---|---|
| 1 | Delete the 13 redundant `DamageStages` sites; README step count; renumber the tutorial steps; `familiar` test helper; record test times in one place | 1, 4 | XS | - | 2.4.2, 6.5; the rest new |
| 2 | Play begins on its own; the 120-frame warning goes | 1 | S | - | 6.1 |
| 3 | Reflection chance as a rule value; spotting and reflection rate tests become tests of the call (4.3) | 4, 3 | S | - | new |
| 4 | `PlaceSeed` in `PlaceRules::build`, `app.places(rules)`, `spawn_player`, `TileAppearance::standard`, the template as a game plugin | 1, 3 | M | - | 1.3 and 6.2, merged so the 12 impls migrate once |
| 5 | One seed spelling, seed modules, `RunRng<D>`, the one re-baseline | 3 | M | 4 | 1.1, 1.2, 1.4, 1.5 |
| 6 | `PlayerKeysPlugin` from step 1; log stamps the turn; factions once | 1 | S each | 4 | 6.3 to 6.5 |
| 7 | `rl-rules` depends on `rl-mapgen`; the eight twins go; `layout.rs` and the Bevy-free prefab adapters move to tier 1 | 2, 3, 4 | M | - | widens 2.4.3; TODO 27 |
| 8 | Foundry's ambiguity test in Undermere's form; engine systems `pub(crate)` | 3 | S | - | new; TODO 28 |
| 9 | Remove `BuildContext`, intern-free `Facets`, delete `Charts` | 3 | M | 7 | 2.1, 2.3, 2.4.1 |
| 10 | Measure Undermere's content load; skip re-checking it in test apps if it dominates | 4 | S | - | new |
| 11 | Duplicate-name check, renames with the collisions fixed, one meaning of "hold" | 3 | M | 7 | branch 4, table amended |
| 12 | Prelude by measured use, topic modules, engine `Rect` renamed, one app trait per crate | 3, 1 | M | 9, 11 | 3.2 and branch 5, merged |
| 13 | Lean Bevy features per tier-2 crate; tile glow read in tier 1 | 2 | S | - | new |
| 14 | `TtyPlugin` and `TtyRoguelikePlugins`, per-user saves, listed in `docs/TODO.md` first | 2 | M | 13 | new |
| 15 | A monster in `examples/terminal`; README toolkit sample on crate paths | 2, 1 | S to M | 7 | new |
| 16 | The place model to tier 1 | 2 | M | 4, 7 | new |
| 17 | Rule numbers and `at_odds` out of systems; split `resolve_items` | 4 | S to M | - | new; TODO 34 |
| 18 | Module maps in rl-bevy's and rl-render's crate docs | 3 | XS | 12 | new |

Items 1 to 3 are cheap and independent and can go first in any order.
Item 4 before 5, because the seed branch would otherwise change `PlaceRules::build` and the hello-world branch change it again.
The plan's order, seeds first and hello world last, puts the largest benefit to goal 1 at the end; nothing in the hello-world tasks depends on the renames or the prelude split, so they can come forward.

## The plan

Where this review agrees with `docs/superpowers/plans/2026-10-20-engine-usability.md`: branches 1 and 2 (except task 2.2), the plan's corrections of the first version, the rejection of cargo features and an `rl-extras` crate, and the Review Focus cases.
Where it should change:

- **Task 2.2, `HitRules` as an enum: drop it.** An enum of hit models is a closed taxonomy, which `CLAUDE.md` forbids ("registries and traits are the only extension mechanisms"); a game's own model, a d20 for example, is exactly what the trait is for; and four test implementations (`D20` in `rl-rules/src/accuracy.rs:298`, `Never` twice and `OnlyAt` in `rl-bevy`) use it as the seam that makes a hit certain or impossible.
  At most, `HitRules::percent(..)` as a constructor.
- **Task 3.1 is done.** `16304359` replaced the message checks with plugin-only markers, which read the same in either plugin order, so the Review Focus order case already holds.
  What is left is 3.7's small note.
- **Task 3.2 belongs with branch 5,** and `AddSettings` is rl-bevy's, not rl-ui's.
- **Tasks 1.3 and 6.2 should be one change,** so the 12 `PlaceRules` impls migrate once.
- **Branch 4's tables need amending:** 26 duplicated names, not 8, of which 8 collapse through `docs/TODO.md` item 27 rather than rename; and three chosen names collide (`Trigger`, `Statuses`, `Placement`).
- **Branch 5's target** should start from measured use: 34 names for step 1, 75 for steps 1 to 3, 84 for the template, and add the tactics, `FactionDef` and a renamed `Rect`.
- **Branch 6 should come before branches 2 to 5,** as the ranking says.
- **The plan's counts:** `set(EngineState::Playing)` is at 14 sites in games, the tutorial and the template and 115 in tests, not 49 outside tests; log calls passing a turn are about 55, not 38; `ResMut<Facets>` sites are 27.
- **Not in the plan:** goal 2 (items 7, 13 to 16) and goal 4 (items 3, 10, 17) are new.

## Corrections to the 2026-10-20 version

- The prelude counts were close but not exact: rl-ui 147, rl-rules 125, rl-mapgen 37, rl-grid 29; the total of about 720 holds (722 entries).
- Eight duplicated names were listed; there are 26, and eight of them are one concept in two tier-1 crates, not overloads.
- The starter template, which the README offers first, was not measured; it is 355 lines of code.
- Step 1's `player_input` is 23 lines, not 35.
- `DamageStages` already defaults to `[SubtractArmor]` and `CombatPlugin` already initializes it; the sites are 13, not about ten. (The plan found the default.)
- Log calls passing the turn are about 55, not 25; `ResMut<Facets>` sites are 27, not 41.
- The presence checks were miscounted as spellings of one question; most `is_plugin_added` sites add a view plugin, and the A2 bug is fixed.
- `AddSettings` is declared in rl-bevy.
- `HitRules` as an enum was suggested; that is a closed taxonomy, against `CLAUDE.md`.
- "Features only Undermere uses" included `accrete`, `dress`, `Offers` and revival, which Foundry uses too; the Undermere-only part is about 6,900 lines, not 12,000.
  `Charts` has no user at all. (The plan found both.)
- The engine's seed domains were said to include `items`; it has none, and has 12 others.
  Undermere's inline domains are at 15 sites, not about twenty, and its "one place keyed two ways" is one integer derived two ways, since its `MapId` is its floor number; the hand-mixed seeds in the tutorial and Foundry were missed.
- The order suggested, seeds first, then prelude and hello world, then features, ranked by area rather than by benefit.

## Complex but justified

These were looked at again on 2026-10-21 and are worth their cost; they need no third review.

- The Bevy-free tiers, with `Brain<A>`, `Snapshot<A>` and `DamageStage<A>` generic over the actor id, so the toolkit runs the same AI and damage without an ECS.
- `Effect` and `FromArgs` as two traits, kept apart for object safety (`rl-bevy/src/effects/mod.rs`): 18 engine and 36 game implementations each.
- `HitModel` as a trait: two engine models, a game's own is the extension, and four test models use it as the seam.
- Traits with many real implementations: `PlaceRules` (12 game impls), `Pass` (about 40 engine and 12 game), `Tactic` (24 and 9), and the `Saveable` traits.
- The turn system sets, needed for ordering across crates; the tutorial touches six (`EngineSet::Input`, `TurnSet::Decide`, `TurnSet::React`, `ResolveSet::Act`, `ViewSet::Annotate`, `PresentSet::Narrate`), and the docs should lead with those.
- The view, collector and presenter split: one `MessageLog` feeds both `LogPanel` and `ScrollbackPanel`.
- The cue and hold mechanism itself, which costs nothing in a headless run; only its name is in question (3.3).
- `rl_engine::testing` and `rl_bevy::testing` as public modules: they are the product a game's tests are written against, not test-only leaks.
- The fingerprint's four scripted delves, 54 to 59 s and a fifth of Undermere's fast tier: a tripwire is not sped up by changing what it plays.
- Named random streams, as in 3.9.
