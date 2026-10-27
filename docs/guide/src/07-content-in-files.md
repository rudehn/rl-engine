# Content in files

> Run it: `cargo run -p tutorial --bin step07_content`
>
> Source: [`step07_content.rs`](https://github.com/rudehn/rl-engine/blob/main/examples/tutorial/src/bin/step07_content.rs)

The rats move out of Rust and into a file, and so do the tiles.
Adding a creature becomes a line of RON.

## The bestiary file

<!-- include: ../../../examples/tutorial/assets/rats.ron -->
```ron
#![enable(implicit_some)]
// What lives in the warren, floor by floor.
//
// Every field:
//   name:       unique; the log calls it this
//   glyph:      one character
//   color:      (r, g, b) in 0..=1
//   hp:         maximum health
//   armor:      flat damage removed from each hit
//   attack:     dice notation, "NdS+B" or a flat number
//   kind:       the damage kind it deals, by the name the game registered: "bite" | "venom"
//   perception: tiles at which it notices you
//   speed:      percent, 100 normal
//   flee_at:    percent health at or below which it runs; 0 never flees
//   clever:     optional; opens doors, throws rocks, and carries a crust it
//               gnaws when hurt
//   shoves:     optional; whether it shoves you back a cell instead of biting
//               when you stand beside it
//   spawn:      (first_floor, last_floor, weight, min_group, max_group);
//               a weight of 0 keeps it out of the table, for anything the
//               game places by hand
[
    (name: "rat",        glyph: 'r', color: (0.72, 0.55, 0.45), hp: 6,  armor: 0, attack: "1d3",   kind: "bite",  perception: 7,  speed: 110, flee_at: 30, spawn: (1, 3, 6, 2, 4)),
    (name: "ratling",    glyph: 'R', color: (0.85, 0.66, 0.50), hp: 6,  armor: 0, attack: "1d3",   kind: "bite",  perception: 7,  speed: 110, flee_at: 25, clever: true, spawn: (1, 4, 3, 1, 2)),
    (name: "grey rat",   glyph: 'r', color: (0.62, 0.64, 0.68), hp: 9,  armor: 1, attack: "1d4",   kind: "bite",  perception: 8,  speed: 100, flee_at: 20, spawn: (2, 4, 4, 1, 3)),
    (name: "root adder", glyph: 's', color: (0.45, 0.78, 0.42), hp: 7,  armor: 0, attack: "1d5+1", kind: "venom", perception: 6,  speed: 130, flee_at: 0,  spawn: (2, 4, 3, 1, 2)),
    (name: "warren hog", glyph: 'h', color: (0.85, 0.60, 0.55), hp: 18, armor: 2, attack: "1d6",   kind: "bite",  perception: 5,  speed: 90,  flee_at: 0,  shoves: true, spawn: (3, 4, 2, 1, 1)),
    (name: "rat king",   glyph: 'R', color: (0.95, 0.78, 0.35), hp: 40, armor: 2, attack: "2d4",   kind: "bite",  perception: 12, speed: 100, flee_at: 0,  spawn: (0, 0, 0, 1, 1)),
]
```

Six kinds of vermin, each with its stats and where it lives.
`spawn` is the first floor, the last floor, a weight, and how many come together.
The rat king has a weight of 0, so it never turns up at random.

## Reading it

<!-- include: ../../../examples/tutorial/src/bin/step07_content.rs:def -->
```rust,no_run
/// One kind of vermin, exactly as `assets/rats.ron` writes it. Serde parses
/// it; [`Named`] says what to key it by; a [`NameRef`] is a name in the
/// file that the load turns into an id.
#[derive(Debug, Clone, Deserialize)]
struct RatDef {
    name: String,
    glyph: char,
    color: (f32, f32, f32),
    hp: i32,
    armor: i32,
    attack: DiceRoll,
    kind: NameRef<DamageKind>,
    perception: i32,
    speed: u32,
    flee_at: i32,
    #[serde(default)]
    clever: bool,
    spawn: (i32, i32, u32, u32, u32),
}

impl Named for RatDef {
    fn name(&self) -> &str {
        &self.name
    }
}
```

`RatDef` mirrors one line of the file, and serde reads it.
`kind` is a `NameRef`: a name in the file that becomes an id when it loads, and fails the load if the name is unknown.

## The bestiary

<!-- include: ../../../examples/tutorial/src/bin/step07_content.rs:bestiary -->
```rust,no_run
/// The brain every kind is built from: bite, run once hurt enough, chase
/// what it sees, else wander.
const WARREN: Standard = Standard { melee: true, flee: true, flee_after_attacks: true, hunt: true, wander_chance: Some(40), ..Standard::NONE };

/// A clever kind's brain adds its knacks, throwing and fetching rocks.
const CLEVER: Standard = Standard { ability_chance: Some(80), throw_chance: Some(60), scavenge_reach: Some(9), wander_chance: Some(30), ..WARREN };

/// What every creature shares that is not in the file.
struct Kin {
    faction: FactionId,
    bite: atglyph::rules::damage::DamageKindId,
    bread: TagId,
    gnaw: AbilityId,
}

/// The bestiary: the kinds, a brain for each, and a table of what lives
/// at which depth.
#[derive(Resource)]
struct Bestiary {
    defs: Registry<RatDef>,
    table: BandedTable<Id<RatDef>>,
    minds: Vec<Arc<Brain<Entity>>>,
    kin: Kin,
}

impl Bestiary {
    fn load(names: &Names, kin: Kin) -> Self {
        let defs: Registry<RatDef> = names.load(RATS_RON).unwrap_or_else(|e| panic!("assets/rats.ron: {e}"));
        let mut table = BandedTable::default();
        let mut minds = Vec::new();
        for (id, def) in defs.iter() {
            let (first, last, weight, group_min, group_max) = def.spawn;
            if weight > 0 {
                table.push(BandedEntry::new(id).bands(first, last).weight(weight).group(group_min, group_max));
            }
            let wits = if def.clever { Wits::SAPIENT } else { Wits::ANIMAL };
            let can = Repertoire { flee_at: def.flee_at, melee: true, abilities: def.clever, wits, ..Repertoire::default() };
            let brain = if def.clever { CLEVER.brain(can) } else { WARREN.brain(can) };
            minds.push(Arc::new(brain.build()));
        }
        Self { defs, table, minds, kin }
    }

    /// Spawns one of `id` at `at`.
    fn spawn(&self, commands: &mut Commands, id: Id<RatDef>, at: Point) -> Entity {
        let def = self.defs.get(id);
        // A clever kind carries its supper, and knows how to eat it.
        let supper = def.clever.then(|| commands.spawn((Item, Crust(6), Tagged(vec![self.kin.bread]), Name::new("crust of bread"))).id());
        let mut e = commands.spawn((
            (Actor, Blocks, Position(at), Speed(def.speed), Faction(self.kin.faction)),
            (Health::full(def.hp), Armor(def.armor), Perception(def.perception), DarkSight(9), Mind(self.minds[id.index()].clone())),
            (MeleeAttack::new(def.kind.id(), def.attack), Glyph::new(def.glyph, Color::srgb(def.color.0, def.color.1, def.color.2)).on_layer(5)),
            (Name::new(def.name.clone()),),
        ));
        if def.clever {
            e.insert((Intelligence(Wits::SAPIENT), Inventory { items: supper.into_iter().collect() }, Grants(vec![self.kin.gnaw])));
        } else {
            e.insert(Intelligence(Wits::ANIMAL));
        }
        e.id()
    }
}
```

`Bestiary` replaces chapter 3's `Rats` resource.
It loads the file into a `Registry`, which gives each line an `Id`, and builds a spawn table and a brain per kind.

The hand-built brains are gone too.
`Standard` is the engine's usual chain of tactics, and a `Repertoire` says what each kind can do.
A `clever` kind gets throwing, scavenging and its knack.

`start` loads it against the registries, so `kind: "venom"` must be a registered damage kind:

<!-- include: ../../../examples/tutorial/src/bin/step07_content.rs:start -->
```rust,no_run
/// Hands combat its registries and the rats their brain, then spawns the
/// player at the first floor's start, lantern open.
fn start(mut commands: Commands, seed: Res<Seed>, effect_kinds: Res<EffectKinds>, mut log: Log) {
    // What damage can be, and who hates whom: the game's content, which
    // the engine knows only by id.
    let kinds = Registry::from_defs(vec![DamageKind::new("bite"), DamageKind::new("venom"), DamageKind::new("kick"), DamageKind::new("din")]).unwrap();
    let sides = Registry::from_defs(vec![FactionDef::new("you"), FactionDef::new("vermin")]).unwrap();
    let (you, vermin) = (sides.expect("you"), sides.expect("vermin"));
    commands.insert_resource(CombatRules::new().hostile(you, vermin));
    // A tag a gnaw's cost names, so a crust can be spent by it.
    let tags = Registry::from_defs(vec![TagDef::new("bread")]).unwrap();
    let bread = tags.expect("bread");
    let registries = Registries { damage_kinds: kinds.clone(), factions: sides, tags, ..default() };
    // Checked against the registries and the effects as it loads, so a
    // misspelt name fails here, at startup.
    let knacks = Abilities::load(KNACKS_RON, &effect_kinds, &registries.names()).unwrap_or_else(|e| panic!("assets/knacks.ron: {e}"));
    let screech = knacks.defs().expect("screech");
    let gnaw = knacks.defs().expect("gnaw");
    commands.insert_resource(Knacks { screech });
    commands.insert_resource(knacks);
    // The bestiary names damage kinds, so it loads against the registries.
    commands.insert_resource(Bestiary::load(&registries.names(), Kin { faction: vermin, bite: kinds.expect("bite"), bread, gnaw }));
    commands.insert_resource(registries);

    commands.spawn_player(
        map_of(1),
        (
            (Viewshed::new(9), RevealsMap, LANTERN, Faction(you), Grants(vec![screech]), Glyph::new('@', Color::WHITE).on_layer(10)),
            (Health::full(24), Armor(1), MeleeAttack::new(kinds.expect("kick"), DiceRoll::new(1, 6)), Inventory::default()),
        ),
    );
    log.notice(format!("Seed {}. You squeeze into the warren.", seed.0.0));
    log.muted("Something is scratching in the dark.");
}
```

## Placing groups

<!-- include: ../../../examples/tutorial/src/bin/step07_content.rs:populate -->
```rust,no_run
/// The random stream rats are placed from. Each floor draws it from its own
/// seed, so floors can be entered in any order.
const RATS: SeedDomain = SeedDomain::new(b"warren.rats");

/// Fills a floor the first time it is entered.
fn populate(
    mut commands: Commands,
    mut entered: MessageReader<PlaceEntered>,
    bestiary: Res<Bestiary>,
    map: Res<WorldMap>,
    seed: Res<Seed>,
    mut tell: MessageWriter<Tell>,
) {
    for ev in entered.read() {
        let depth = floor_of(ev.map);
        tell.write(Tell::new(format!("Floor {depth}: {}.", name_of(depth)), Tones::NOTICE));
        if !ev.first {
            continue;
        }
        // The stairs are the engine's. The way out, at the last floor's far
        // point, is an entity the game spawns.
        if let Some(far) = ev.exit.filter(|_| depth == FLOORS) {
            commands.spawn((Position(far), WayOut, Name::new("crack of daylight"), Glyph::new('<', Color::srgb(1.0, 0.95, 0.70)).on_layer(1)));
            // The rat king sits on it. Its weight of 0 keeps it out of the
            // table, so it is placed by name.
            bestiary.spawn(&mut commands, bestiary.defs.expect("rat king"), far);
        }
        let Some(place) = map.place(ev.map) else { continue };
        let bounds = place.terrain.bounds();
        let mut rng = seed.place(ev.map).rng(RATS, 0);
        // Groups, not single rats: the table says what may live at this
        // depth, how likely it is, and how many come together.
        let mut groups = 0;
        for _ in 0..60 {
            if groups >= 3 + depth as usize {
                break;
            }
            let Some((&id, count)) = bestiary.table.pick_group(depth as i32, &mut rng) else { break };
            let anchor = Point::new(rng.random_range(bounds.x..bounds.right()), rng.random_range(bounds.y..bounds.bottom()));
            let mut placed = 0;
            for p in geometry::square(anchor, 2) {
                // On floor, and not close enough to the start to be unfair.
                if placed < count && map.is_walkable(p) && geometry::chebyshev(p, ev.entry) >= 8 {
                    bestiary.spawn(&mut commands, id, p);
                    placed += 1;
                }
            }
            if placed > 0 {
                groups += 1;
            }
        }
        // Bread and rocks, scattered the same way.
        let mut dropped = 0;
        while dropped < 14 {
            let p = Point::new(rng.random_range(bounds.x..bounds.right()), rng.random_range(bounds.y..bounds.bottom()));
            if map.is_walkable(p) {
                litter(&mut commands, &bestiary, p, dropped % 2 == 0);
                dropped += 1;
            }
        }
    }
}
```

`pick_group` draws a kind that may live at this depth, by weight, and how many come together.
The rat king is placed by name, on the crack of daylight.

## The tiles

<!-- include: ../../../examples/tutorial/assets/tiles.ron -->
```ron
// The warren's tiles: what each one is, and how it looks in full light.
// The renderer works out darkness and memory from the two colours.
//
// Every field (the ones marked "optional" may be left out):
//   tile:               the tile's name, what the game and other lines call it
// What it is:
//   walkable:           optional; whether you can stand on it; false
//   opaque:             optional; whether it blocks sight; false
//   blocks_projectiles: optional; whether a thrown thing stops at it; false
//   passable:           optional; whether a route may go through it later, as a
//                       shut door is; as walkable when left out
//   move_cost:          optional; hundredths of a step to enter; 100
//   opens_to:           optional; the tile it becomes when opened, by name
//   closes_to:          optional; the tile it becomes when closed, by name
//   burn:               optional; (catch_pct:, turns:, leaves:), how it burns
//   light:              optional; (intensity:, radius:, color: (r:, g:, b:))
// How it looks:
//   glyph:              one character
//   fg:                 (r, g, b) in 0..=1, the glyph's colour
//   bg:                 optional; (r, g, b), the cell's fill; black
//   vary:               optional; (brightness, hue), how far each cell strays
//                       from the authored colour, 0..=1 each; none
//   shimmer:            optional; brightness drifting over time, 0..=1
[
    (tile: "earth", opaque: true, blocks_projectiles: true, glyph: '#', fg: (0.78, 0.66, 0.50), bg: (0.34, 0.27, 0.21), vary: (0.20, 0.05)),
    (tile: "dirt",  walkable: true, glyph: '.', fg: (0.66, 0.58, 0.45), bg: (0.18, 0.15, 0.12), vary: (0.28, 0.06)),
    // Walkable, but you cannot see through it.
    (tile: "roots", walkable: true, opaque: true, glyph: '+', fg: (0.55, 0.74, 0.45), bg: (0.16, 0.22, 0.13), vary: (0.18, 0.05)),
]
```

<!-- include: ../../../examples/tutorial/src/bin/step07_content.rs:looks -->
```rust,no_run
    /// What each tile is and how it looks, read from `assets/tiles.ron`.
    fn new() -> Self {
        Self { tiles: atglyph::rules::tiles::load(TILES_RON).unwrap_or_else(|e| panic!("assets/tiles.ron: {e}")) }
    }
```

Each line says everything about a tile: walkable, opaque, and the rest, then its look.
`atglyph::rules::tiles::load` reads the file into the same `TileRegistry` the code built before, looks included, so `appearance` doesn't change.
A mistake in the file, such as a door that opens into a tile the file doesn't define, is reported at startup, by name.

## Try it

- Add a creature to `rats.ron` and see it on the floors you gave it.
- Misspell a damage kind and read the error.

Next: [an action of your own](08-an-action-of-your-own.md).
