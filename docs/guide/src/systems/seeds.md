<!-- documents:
     plugins: none
     files: crates/atglyph-bevy/src/seed.rs
            crates/atglyph-bevy/src/replay.rs
            crates/atglyph-core/src/seed.rs
     fingerprint: d2c2ab87 -->

# Seeds and determinism

A run has one seed, every place in it a seed of its own derived from it, and everything random is derived from one of those by name.
A game inserts a `Seed` and nothing else: each subsystem that rolls owns a generator under a domain of its own and derives it for itself, so adding a subsystem adds no line to a game and one subsystem's draws never shift another's.
The same seed and the same build give the same run, which is what makes a replay a replay and a failing test a bug rather than a mood.
Determinism is promised within one build of a game, not across engine versions.

## Turning it on

There is no plugin here either.
`Seed(RunSeed(n))` is a resource the game inserts before play begins, and `LaunchArgs::from_env()` is the reading of it off wasm, in three steps.
The seed of the recording `ATGLYPH_REPLAY` names comes first, whatever the command line says, because a replay is the run it was recorded from and a replay on a fresh seed drifts on its first key; then `--seed N` from the command line, refusing a value that is not a whole number by name; then none, which `seed_or_fresh` turns into a fresh seed.
`LaunchArgs::rest` is every other argument, the recording's when replaying, so a game's own flags replay with it; `LaunchArgs::parse` is the same reading without a process, and `Seed::from_args()` is the seed alone, for a game with no flags of its own.
A plugin that rolls calls `app.add_stream::<S>("MyPlugin")` in its own `build`, which declares `needs::<Seed>` with that plugin named, so a game that forgot the seed is told which subsystem wanted it.
`CombatPlugin`, `AbilitiesPlugin`, `MindsPlugin`, `PropsPlugin`, `LootPlugin`, `EffectsPlugin` and `StealthPlugin` each do this, and two plugins asking for the same stream get one deriving system between them.
A game that adds none of those still inserts a `Seed` if anything of its own draws, because its draws come from the resource.

## The model

`RunSeed(pub u64)` is the root, and `derive(domain, index)` is `mix64(mix64(seed ^ domain.salt()) + index)`, a `const fn` depending on all three inputs, so a new run reshuffles every index of every domain at once and two domains never share a stream.
`SeedDomain::new(b"name")` is FNV-1a over the name with the low bit forced on, and it is a struct rather than an enum, so a game declares `const WEATHER: SeedDomain = SeedDomain::new(b"weather")` without editing the engine.
The name is what keys a stream, so renaming a domain rerolls it and reordering declarations does not.
A name starting `atglyph.` is the engine's, made with `SeedDomain::engine`, and `SeedDomain::new` refuses one, a compile error in a `const`; `ENGINE_DOMAINS` lists every engine domain by name, so a game's test can check its own against them.
`RunSeed::rng(domain, index)` is `StdRng::seed_from_u64` over that derivation, the one spelling for a draw; `StdRng` and not `SmallRng`, because the algorithm has to be the same in a browser as on the desktop.
`PlaceSeed` is the seed of one place, `PlaceSeed::of(run, map)`, derived under `atglyph.place` by the place's number alone, so building floor 2 before floor 1 cannot change floor 1, and a generator handed one cannot reach the run's other streams; its `rng(domain, index)` draws inside the place, and `run_seed` is it as a root a generation chain starts from.
`Seed(pub RunSeed)` is the Bevy resource, and `seed.place(map)` is that place's `PlaceSeed`: a game's draws about a place come from `seed.place(map).rng(DOMAIN, index)`, and its draws about the whole run from `seed.0.rng(DOMAIN, index)`.
`Stream` is the trait a subsystem's generator implements, with one method, `for_run(seed) -> Self`; `RunRng<D>` implements it for any `StreamDomain` marker `D`, which is where the domain is written down, so a subsystem's stream is an alias such as `CombatRng`.
`add_stream::<S>` inserts a marker so a second registration adds nothing, and adds `derive_stream::<S>`, which runs on `resource_exists_and_changed::<Seed>` before `EngineSet::Stream`, not gated on play.
So a stream is derived before the first frame that needs it, and derived again the moment the seed changes, which is how a game that learns its seed late, from a save it is continuing, gets streams that match the run it is resuming.
`RunSeed::fresh` mixes the wall clock with a per-process counter, and is not compiled on wasm, which has no `SystemTime`; `from_entropy` takes a reading the host supplies instead.
For a draw that must not depend on visiting order inside a place there are `position_hash` and `pair_hash`, which depend only on their inputs, so iterating cells in a different order cannot reroll them.

## Using it

The tutorial's spawner names its domain once and draws from the floor's own seed.

<!-- include: ../../../../examples/tutorial/src/bin/step03_blows.rs:populate -->
```rust,no_run
/// The stream the rats are placed from, named once and drawn from each
/// floor's own seed: a spawner added later draws from a stream of its own
/// and cannot shift what this one places, and the floors may be entered in
/// any order.
const RATS: SeedDomain = SeedDomain::new(b"warren.rats");

/// Fills the floor the one time it is built. `PlaceEntered::first` is
/// true only on that arrival, so coming back does not restock it.
fn populate(mut commands: Commands, mut entered: MessageReader<PlaceEntered>, rats: Res<Rats>, map: Res<WorldMap>, seed: Res<Seed>) {
    for ev in entered.read() {
        if !ev.first {
            continue;
        }
        let Some(place) = map.place(ev.map) else { continue };
        let bounds = place.terrain.bounds();
        let mut rng = seed.place(ev.map).rng(RATS, 0);
        let mut placed = 0;
        while placed < 16 {
            let p = Point::new(rng.random_range(bounds.x..bounds.right()), rng.random_range(bounds.y..bounds.bottom()));
            // Not on top of the player, and not close enough to be unfair.
            if !map.is_walkable(p) || geometry::chebyshev(p, ev.entry) < 8 {
                continue;
            }
            commands.spawn((
                (Actor, Blocks, Position(p), Speed(110), Faction(rats.faction)),
                (Health::full(6), Armor(0), Perception(7), DarkSight(9), Mind(rats.mind.clone()), Glyph::new('r', Color::srgb(0.72, 0.55, 0.45)).on_layer(5)),
                (MeleeAttack::new(rats.bite, DiceRoll::new(1, 3)), Name::new("rat")),
            ));
            placed += 1;
        }
    }
}
```

## The line

A subsystem's stream is a `Stream` registered with `add_stream`, derived from the run's `Seed` through `RunSeed::derive(domain, index)`, and never anything else.
A game's own draws come from `RunSeed::rng` and `PlaceSeed::rng`, named and indexed the same way, which is why a spawner added in the tenth week cannot shift what a spawner written in the first one places.
No engine crate builds a generator from a constant, and none seeds one straight from entropy: every generator comes from a `RunSeed`, and `RunSeed::fresh` is the one function in the engine that turns a clock into a seed, off wasm only, for a run nobody named a seed for.
A function that rolls takes `&mut impl Rng` rather than making one, so the caller decides which stream it came from and a test can hand it a fixed one.
The engine decides how a domain and an index become a generator and when a stream is rebuilt; the game decides what its domains are called, what they are indexed by, and where its seed came from.
Choosing a seed and an index is the game's judgement and it matters: drawing from a place's seed gives a floor the same contents whether or not the player dawdled on the one above, and keying by a counter does not.
A replay is a seed plus a build, so a change to a draw order in engine code is a change to every recorded run, which is why determinism is promised within a build and not across versions.

## Where it lives

`atglyph-core` is tier 0: `RunSeed`, `PlaceSeed`, `SeedDomain`, `mix64`, `position_hash` and `pair_hash` are all `const fn` over plain integers, so what a derivation gives for a seed and a domain is proved by arithmetic with no `App` and no `World` anywhere near it.
That is also why the same derivation runs on `wasm32-unknown-unknown`, where `atglyph-core` has to build and where there is no clock to fall back on.
`atglyph-bevy` adds only the wiring: `seed.rs` is the `Seed` resource, `LaunchArgs`, the engine's domains, the `Stream` trait, `add_stream` and the one system that derives.
Keeping the arithmetic a tier below the wiring is what lets a seeding property be tested over a range of seeds rather than over a range of frames.
