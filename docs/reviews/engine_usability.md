# Engine usability review

Written 2026-10-20, at main 49c4549a, from a read-only survey of the code and the tutorial.
The question was which parts are over-engineered, and what would confuse a game author writing a new small roguelike from `docs/guide` and `examples/tutorial`.
Counts are from that survey; re-check them before acting, since eight fix branches were in flight when it was taken.
Nothing here is scheduled yet.
The suggested order is the seed cleanup first, since it is small and self-contained, then the prelude and the hello-world setup, then moving Undermere-only features behind features.

## What confuses a new game author

### 1. The prelude is about 720 names

`rl_engine::prelude` (`crates/rl-engine/src/lib.rs`) globs every crate's prelude: rl-bevy 334, rl-ui 146, rl-rules 124, rl-mapgen 35, rl-grid 28, rl-render 18, rl-save 17, rl-core 14.
It carries internal plumbing such as `Airborne`, `Lands`, `AddAirborne`, `Charting`, `Marksmanship` and `SideWas`.
Some names mean different things in different crates: `Reach` four times (rules `arms.rs`, rules `prefab.rs`, bevy `combat.rs`, ui `view/target.rs`), `Turn` three times, and `Offered`, `Cell`, `Spent`, `Heard`, `Secret` and `Placed` twice each.
Suggested: a core prelude of about 60 names covering the tutorial's first three steps, and opt-in `prelude::combat`, `prelude::items` and the like.

### 2. Hello world takes too much setup

Tutorial step 1 inserts four resources (`Seed`, `TileAppearance`, `WorldMap::new(tiles.tables())`, `PlaceRulesRes(Box::new(..))`), spawns the player at `Point::ZERO` and then warps it with `WarpRequest::into_place`, and hand-defines `MapId(1)`.
It writes a 35-line `player_input` although `PlayerKeysPlugin` exists; only step 10 uses it.
Every tutorial step and both games call `next.set(EngineState::Playing)`, and `plugin.rs` warns after 120 frames when it is forgotten, which says it bites.
By step 5 the setup is thirteen `insert_resource` calls.
`MessageLog::push` and `notice` take the turn number explicitly, at about 25 call sites.
Suggested: `app.place_rules(..)` builds the `WorldMap` from the registry; play begins on its own after a new run unless a `HoldStart` holds it; a `spawn_player(map)` helper; `PlayerKeysPlugin` from step 1; the log stamps the turn itself.
The guide also skews: chapters 7 and 8 run `step08` and `step09`, there is no `step07`, and the README says six steps where there are nine binaries.

### 3. Combat setup is copied around

`DamageStages(vec![Box::new(SubtractArmor)])` is written out at about ten sites, each with the same stage; only Undermere adds one of its own (`HalveArmor`).
Factions are passed twice, into `Registries` and into `CombatRules::new(&sides)`.
Suggested: a default stage list, and `CombatRules` reading the factions from `Registries`.

### 4. Several ways to say a plugin is running

`depends_on` asserts (about 60 sites), `is_plugin_added` guards (18), `app.needs` (26), `app.reads` (14), and the `NoiseRunning`, `StealthRunning` and `MindsRunning` checks that read whether a message queue exists.
The last are review bug A2 in `engine_api.md`, fixed on the `fix-running` branch.
There are also about twenty single-use `App` extension traits: `AddAction`, `AddChoice`, `AddEffect`, `AddMoment`, `AddVerb`, `AddWork`, `AddSound`, `AddSpending`, `AddStream`, `AddTone`, `AddModal`, `AddSettings`, `AddSaveable`, `AddAirborne`, `AddControls`, `AddEngineEffects`, `KeepOnRemnant`, `Needs`, `Reads`, `ResetsOnNewRun`.
Suggested: one presence marker per plugin, one declarative way to state needs and reads, and the extension traits merged into one.

### 5. Opaque and overloaded names

"Hold" means five things: `TurnHold` waits for animations, `HoldStart` lets a title screen delay the run, `StatusDef::holds()` is a stun, `HeldWhileWorn` is statuses granted by gear, and `held_for_life` is innate statuses.
`Charts::holds` means "contains", and key repeat is described as "the engine's hold".
The ten most opaque public names, and what each is:

- `Loadout`: a SystemParam working out effective stats and attacks from an actor and its worn gear.
- `Facet`: a free-text note added to a UI row.
- `Cue` and `Cued`: a request to the renderer to animate a projectile or a burst.
- `TurnHold`: pauses the turn loop while animations play.
- `Moment`: the kind of event that fires an effect trigger, such as on use, on hit or on step.
- `Afflict`, `Afflicted`, `Cure`: the message applying a status, the component listing statuses, the message removing one.
- `Bestows` and `Strikes`: stat changes from worn gear, and extra damage dice on each hit.
- `Among` and `Chooser`: a callback choosing which carried items an offer accepts.
- `Keen`, `Provenance`, `Pulse`: a bonus to spotting hidden things, the context and depth an item was made at, a timer firing a trigger on worn gear.
- `Stowed`, `Life`, `Doing`: an attack that cannot be used now, a snapshot of a dead actor kept for revival, the name of an AI's current tactic.

The place vocabulary needs a sentence of glossary: `MapId` names a place, and `PlaceMap`, `WorldMap`, `PlaceBuild`, `PlaceRules`, `PlaceRulesRes`, `OnMap` and `WindowView` hang off it.
Suggested: rename the worst, and give the guide a glossary page.

## Over-engineered

### 6. Generic code with one user

`BuildContext` (`rl-mapgen/src/context.rs`) has one implementation, `BaseContext`; every game pass is `impl Pass<BaseContext>` and game data travels through `emit<T: Any>` rather than a wrapped context, so the parameter can go.
`Facets` interns its keys, which are only filtered in `nearby.rs`, so about 41 example call sites take `ResMut<Facets>` to add a line of text; `&'static str` keys would do.
`HitRules(Box<dyn HitModel>)` has two real implementations, `Certain` and `Percent`, both games use `Percent`, and `D20` exists only in a test; an enum would do.
Rules are held three ways: plain structs (`CombatRules`, `FireRules`, `TurnRules`, `NoiseRules`, `BumpRules`, `SpotRules`), boxed trait objects (`HitRules`, `PlaceRulesRes`) and a list of boxes (`DamageStages`).
Names are registered three ways: eighteen `Registry<Def>` types, six interner wrappers (`Tones`, `Facets`, `Verbs`, `Sounds`, `WorkKinds`, `Moments`) and seventeen `*Id` aliases.
`Charts` (`charts.rs`, 332 lines, with minds and save hooks) has no use in any example and overlaps `Knowledge`.

### 7. Features only Undermere uses, in the engine at full generality

About 12,000 lines: trade (bevy 774, rules 172, panel 556 and its view), records (433, 210 and a 465-line panel), the party and sheet panels and views (about 1,290), `accrete` with recipes and districts (1,974), `dress` (1,032 and 509), `Lakes`, `pool` (1,566, used once by Foundry), namegen and names (1,135), balance and threat, `Offers`, `Among` and `Chooser` (the offers panel is 814), `Keen`, `Reflects`, and `Life` with revival.
Each is general, but a new game author reads past all of it.
Suggested: cargo features or an `rl-extras` crate, and out of the core prelude.

## The random seeds

The design is right at its core: one `RunSeed` per run, and an independent named stream per purpose, so a draw added in one system never shifts another's, which is what keeps saves, replays and the fingerprints stable.
What confuses is the spelling and the keying of a place's draws.

- **Four spellings for one idea.** `Seed::stream(b"name", i)` returns a `StdRng`; `RunSeed::stream(domain)` returns a `u64` under the same name; `RunSeed::rng(SeedDomain::new(b"name"), i)` returns a `StdRng`; `RunSeed::derive(domain, i)` returns a `u64`. Undermere mixes the first and the third.
- **Domains are byte strings typed inline at about twenty Undermere call sites**, such as `undermere.population`, `undermere.vaults.marks` and `lore`. A typo quietly makes a new stream, nothing lists them, and a game's names share one namespace with the engine's (`combat`, `loot`, `items`).
- **One place is keyed two ways.** `recipe`, `biome` and the population streams key on the floor number from the run seed; `place`, `vaults.fixtures`, `vaults.marks` and `writings` key on the `MapId`; generation inside a chain draws from a third root, the place seed `undermere.place` derives from the `MapId` (`examples/undermere/src/dungeon.rs`). One floor's randomness comes from three roots under two keys.
- **Some streams are split finer than they need to be.** One floor's population uses `population`, `population.bands` and `population.plan`; the split is worth keeping only where each must stay stable while the others change, and no test pins that.
- **The engine's eight streams each hand-write the same `Stream` impl** (`CombatRng`, `MindRng`, `LootRng`, `StealthRng`, `PropRng`, `EffectRng`, `ReflectRng`, `StumbleRng`); a generic `RunRng<D>` keyed by a domain constant would replace them.

Suggested:

1. One spelling, `seed.rng(DOMAIN, index)` taking a `SeedDomain` constant; drop the byte-slice form and rename the `u64`-returning `stream` to `seed_for`.
2. Each game declares its domains as constants in one `seeds` module, with a test that they are distinct; the engine's own domains take an `rl.` prefix so no game can collide with them.
3. One seed per place: the engine hands `PlaceRules::build` a `PlaceSeed` derived from the `MapId`, and every per-place draw comes from it (`place.rng(POPULATION)`), which ends the floor-or-`MapId` mix.

It re-baselines Undermere's and Foundry's fingerprints once.

## Complex but justified

These were looked at and are worth their cost; they need no second review.

- The Bevy-free tiers, with `Brain<A>` and `DamageStage<A>` generic over the actor id so the toolkit stays Bevy-free.
- `Effect` and `FromArgs` as two traits, kept apart for object safety (`effects/mod.rs`), with about 36 game implementations each.
- Traits with many real implementations: `PlaceRules` (12 game impls), `Pass` (35 engine and 12 game), `Tactic` (24 and 9), and the `Saveable` traits.
- The turn system sets, needed for ordering across crates; the docs should lead with the six the tutorial touches (`Input`, `Decide`, `React`, `Act`, `Annotate`, `Narrate`).
- The view, collector and presenter split: one `MessageLog` feeds both `LogPanel` and `ScrollbackPanel`.
- The cue and hold mechanism itself, which costs nothing in a headless run.
- Named random streams, as above.
