<!-- documents:
     plugins: none
     files: crates/atglyph-bevy/src/registries.rs
            crates/atglyph-rules/src/content/registry.rs
            crates/atglyph-rules/src/names.rs
            crates/atglyph-core/src/id.rs
            crates/atglyph-bevy/src/noise.rs
     fingerprint: 7f43eb69 -->

# Registries and content

A registry is every definition of one kind, held in file order and addressable by a dense typed id or by name.
`Registries` is the one resource holding the eight the engine's own subsystems read: damage kinds, factions, stats, statuses, tags, slots, gases and props.
A game fills them before play begins, and loads its own content against them, so a name in a file becomes an id once, at startup, and nothing compares strings during play.
The engine never learns what any of those ids mean.

## Turning it on

There is no plugin here, and no system.
`Registries` is a resource the game inserts, and a plugin that reads one says so with `needs::<Registries>` and a hint naming which registry it wants filled.
`GasPlugin`, `FirePlugin`, `AbilitiesPlugin`, `PropsPlugin`, `LootPlugin`, `PrefabPlugin` and three of `atglyph-ui`'s view plugins all declare it, and every one that is missing is reported together when play begins.
A registry a game has no use for stays empty, and empty means none: no statuses means no badges on a health bar, no slots means nothing is worn.
Registries name each other, so a game fills them in the order they refer to one another, the ones that name nothing first and the ones loaded through `Registries::names` after.

## The model

`Registry<T>` holds a `Vec<T>` and an `Interner<T>`, and issues ids densely in the order definitions arrive, so a `Vec` indexed by id is a valid per-definition table.
`from_defs` takes them in order, `from_ron_str` parses a RON list, and `push` adds one; each refuses a duplicate name with `ContentError::Duplicate`.
`get(id)` indexes straight into the definitions, so it panics only when the id is past the end: an id of the same type from a different registry is in range and gives the wrong definition rather than failing.
`try_get` answers `Option` instead, `id(name)` answers `Option`, and `expect(name)` panics, for names the game knows it shipped.
`validate` runs a check over every definition and collects every failure, so a file with three typos reports three.
`Named` is the one thing a definition type implements: it answers with its unique name, and that is the whole of what the engine asks of a game's own type.
`Names` borrows whichever registries exist, the engine's and a game's alike, and `Names::load` reads a file through it: a `NameRef<T>` field in the RON becomes an `Id<T>` at load, and a name looked up in a registry that was never given is reported as unknown rather than panicking.
`Names::load_list` reads a plain list the same way, for rows that name content without being content: a spawn table has a row per band for one monster, so it is not a `Registry`, and its errors are reported by row.
`Names::interned` puts a vocabulary in scope that a subsystem interns while the app is built rather than loads from a file, looked up and misspelled exactly as a registry's are, so content naming a sound fails at load the way content naming a status does.
`Registries::names` builds that view over seven of its own fields, every one but `props`, which is itself loaded through it, so a game loads its content against exactly the tables the engine will read it with; the sounds are not among them, and a game whose effects make a noise chains `SoundNames::sounds` onto it.
The fields of `Registries` are public and ordinary, so filling one is assignment and there is no builder to learn.
Nothing here is a plugin, a system or a schedule; a registry is data a game hands over before `EngineState::Playing`.

## Using it

The tutorial's start, in `NewRun`, fills the two registries combat reads, loads its own bestiary against them, and hands the resource to the engine before it spawns the player.

<!-- include: ../../../../examples/tutorial/src/bin/step07_content.rs:start -->
```rust,no_run
/// Hands the engine what combat reads, then spawns the player into the
/// warren, at the first floor's entry.
fn start(mut commands: Commands, seed: Res<Seed>, mut log: Log) {
    // The two registries combat reads: what damage can be, and who hates
    // whom. Both are the game's content, named nowhere in the engine.
    let kinds = Registry::from_defs(vec![DamageKind::new("bite"), DamageKind::new("venom"), DamageKind::new("kick")]).unwrap();
    let sides = Registry::from_defs(vec![FactionDef::new("you"), FactionDef::new("vermin")]).unwrap();
    let (you, vermin) = (sides.expect("you"), sides.expect("vermin"));
    commands.insert_resource(CombatRules::new().hostile(you, vermin));
    let registries = Registries { damage_kinds: kinds.clone(), factions: sides, ..default() };
    // The bestiary names the damage each creature deals, so it loads against
    // the registries before they are handed to the engine.
    commands.insert_resource(Bestiary::load(&registries.names(), vermin));
    commands.insert_resource(registries);

    commands.spawn_player(
        map_of(1),
        (
            (Viewshed::new(9), RevealsMap, Faction(you), Glyph::new('@', Color::WHITE).on_layer(10)),
            (Health::full(24), Armor(1), MeleeAttack::new(kinds.expect("kick"), DiceRoll::new(1, 6))),
            (Inventory::default(),),
        ),
    );
    log.push(format!("Seed {}. You squeeze into the warren.", seed.0.0), Tones::NOTICE);
}
```

## The line

Content is an id in a registry.
There is no closed taxonomy enum for content anywhere in the engine: no `DamageKind` variant list, no `enum Faction`, no set of statuses a game picks from, because a list the engine ships is a list a game forks to add its third entry.
`#[non_exhaustive]` with a `Custom { id }` variant is explicitly not how this engine extends, since it keeps the shipped names privileged and makes everyone else's content a special case at every match.
The two extension mechanisms are a registry and a trait, and that is all of them.
The engine decides that ids are dense, that duplicates are refused, and that an unknown name fails at load with the name in the message; the game decides what definitions exist, what fields they carry, and what any of it means.
`Registries` holds the registries the engine's own subsystems read, and a game's own kinds go in a resource of its own, loaded through the same `Names` so a cross-reference between the two resolves.
A registry is filled once, before play, and read from then on: nothing in the engine adds a definition during a run.

## Where it lives

`atglyph-rules` is tier 1 and has no Bevy in it: `Registry`, `Named`, `ContentError`, `Names` and `BandedTable` are there, so loading a content file, refusing a duplicate and resolving a `NameRef` are all tested with plain values and no `App`.
`atglyph-core` is tier 0 and owns `Id<T>` and the `Interner` the dense ids come from, which is why an id is typed and cannot be handed to the wrong registry without the compiler saying so.
`atglyph-bevy` adds exactly one thing on top: `registries.rs`, which is the `Registries` resource and the `names` view over it, and nothing else.
That the Bevy layer's whole contribution is a struct of eight public fields is the point: what a game loads and how it validates belong to a tier that can be tested without a frame.
