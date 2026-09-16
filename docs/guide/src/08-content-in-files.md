# Content in files

> Run it: `cargo run -p tutorial --bin step08_content`
>
> Source: [`step08_content.rs`](https://github.com/rudehn/rl-engine/blob/main/examples/tutorial/src/bin/step08_content.rs)

Warren has one kind of rat because a `spawn` call was hard-coded.
Move the bestiary into a file and the game stops needing a recompile to gain a monster.

## The file

Every RON schema in this repository lists its full option space at the top, because the file is the interface.

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
//   shoves:     optional; whether it shoves you back a cell instead of biting
//               when you stand beside it
//   spawn:      (first_floor, last_floor, weight, min_group, max_group);
//               a weight of 0 keeps it out of the table, for anything the
//               game places by hand
[
    (name: "rat",        glyph: 'r', color: (0.72, 0.55, 0.45), hp: 6,  armor: 0, attack: "1d3",   kind: "bite",  perception: 7,  speed: 110, flee_at: 30, spawn: (1, 3, 6, 2, 4)),
    (name: "grey rat",   glyph: 'r', color: (0.62, 0.64, 0.68), hp: 9,  armor: 1, attack: "1d4",   kind: "bite",  perception: 8,  speed: 100, flee_at: 20, spawn: (2, 4, 4, 1, 3)),
    (name: "root adder", glyph: 's', color: (0.45, 0.78, 0.42), hp: 7,  armor: 0, attack: "1d5+1", kind: "venom", perception: 6,  speed: 130, flee_at: 0,  spawn: (2, 4, 3, 1, 2)),
    (name: "warren hog", glyph: 'h', color: (0.85, 0.60, 0.55), hp: 18, armor: 2, attack: "1d6",   kind: "bite",  perception: 5,  speed: 90,  flee_at: 0,  shoves: true, spawn: (3, 4, 2, 1, 1)),
    (name: "rat king",   glyph: 'R', color: (0.95, 0.78, 0.35), hp: 40, armor: 2, attack: "2d4",   kind: "bite",  perception: 12, speed: 100, flee_at: 0,  spawn: (0, 0, 0, 1, 1)),
]
```

## The struct

<!-- include: ../../../examples/tutorial/src/bin/step08_content.rs:def -->
```rust,no_run
/// One kind of vermin, exactly as `assets/rats.ron` writes it. Serde
/// parses the file; [`Named`] is how the registry knows what to key it by,
/// and a [`NameRef`] is a name in the file that the load turns into an id.
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
    spawn: (i32, i32, u32, u32, u32),
}

impl Named for RatDef {
    fn name(&self) -> &str {
        &self.name
    }
}

/// What an entity on the map was spawned from.
#[derive(Component, Clone, Copy)]
struct Kind(Id<RatDef>);
```

`Named` tells the registry what to key an entry by.
`DiceRoll` deserializes straight from `"1d5+1"`.
`NameRef<DamageKind>` is the interesting one: in the file it is a name, `kind: "venom"`, and by the time the game holds a `RatDef` it is the id of a damage kind.

## Names become ids at load

<!-- include: ../../../examples/tutorial/src/bin/step08_content.rs:bestiary -->
```rust,no_run
/// The bestiary: the defs, one brain per def, and the table that says
/// what belongs at what depth.
#[derive(Resource)]
struct Bestiary {
    defs: Registry<RatDef>,
    table: BandedTable<Id<RatDef>>,
    minds: Vec<Arc<Brain<Entity>>>,
    faction: FactionId,
}

impl Bestiary {
    /// Reads the file against `names`, builds a brain for each entry from its own fields,
    /// and bands the ones with a weight into the spawn table.
    fn load(names: &Names, faction: FactionId) -> Self {
        let defs: Registry<RatDef> = names.load(RATS_RON).unwrap_or_else(|e| panic!("assets/rats.ron: {e}"));
        let mut table = BandedTable::default();
        let mut minds = Vec::new();
        for (id, def) in defs.iter() {
            let (first, last, weight, group_min, group_max) = def.spawn;
            if weight > 0 {
                table.push(BandedEntry::new(id).bands(first, last).weight(weight).group(group_min, group_max));
            }
            let mut brain = Brain::new().then(MeleeAdjacent);
            if def.flee_at > 0 {
                brain = brain.then(FleeWhenHurt { at_pct: def.flee_at });
            }
            minds.push(Arc::new(brain.then(Hunt).then(Wander { chance_pct: 40 })));
        }
        Self { defs, table, minds, faction }
    }

    /// Spawns one of `id` at `at`.
    fn spawn(&self, commands: &mut Commands, id: Id<RatDef>, at: Point) -> Entity {
        let def = self.defs.get(id);
        commands
            .spawn((
                (Actor, Blocks, Kind(id), Position(at), Speed(def.speed), Faction(self.faction)),
                (Health::full(def.hp), Armor(def.armor), Perception(def.perception), Mind(self.minds[id.index()].clone())),
                (MeleeAttack { kind: def.kind.id(), dice: def.attack }, Glyph::new(def.glyph, Color::srgb(def.color.0, def.color.1, def.color.2)).on_layer(5)),
                // What the narrator, and later the rail, call it.
                (Name::new(def.name.clone()),),
            ))
            .id()
    }
}
```

`names.load` parses and checks: names unique, every entry well formed, and every name of other content found.
It fails at start-up naming the file, not three floors down.

The names come from `Registries`, the resource `start` fills with the damage kinds and the sides before anything is loaded against them:

```rust
    commands.insert_resource(Bestiary::load(&registries.names(), vermin));
    commands.insert_resource(registries);
```

Misspell a kind and the run stops at start-up with `root adder: unknown damage kind "vemon"`, every unknown name in the file reported at once under the creature it is in.
They are the same tables the engine reads, so the kind a file was checked against is the kind combat mitigates.
A game's own registries join the lookup the same way: `names.with("item", &items)` lets a creature name what it drops.
Nothing in `RatDef` is a string that could still be wrong, and nothing spawns a rat by looking a name up.

What comes back is an `Id<RatDef>`: a dense index, so `minds[id.index()]` is an array lookup, and typed, so it cannot be passed where an `Id<ItemDef>` belongs.

Note what the loop does with `flee_at`.
It builds a different brain per definition, from that definition's fields, so a rat that never flees gets a brain with no `FleeWhenHurt` in it rather than one that checks a flag every turn.
Content driving structure, not just numbers.

## Bands say what belongs where

```rust
                table.push(BandedEntry::new(id).bands(first, last).weight(weight).group(group_min, group_max));
```

```rust
            let Some((&id, count)) = bestiary.table.pick_group(depth as i32, &mut rng) else { break };
```

A `BandedTable` entry has a depth range, a weight and a group size, so the difficulty curve is four numbers per line in a file.
Rats thin out below floor three, hogs only appear on the last two, adders come in ones and twos.

Weight zero keeps an entry out of the table, which is how the king lives in the same file and is still placed by hand:

```rust
                bestiary.spawn(&mut commands, bestiary.defs.expect("rat king"), throne);
```

`rl-rules` ships a threat score and a band report over this same table, which is what `cargo run -p corsair -- --balance` prints.

## The tiles too

The largest block of Rust in [chapter 1](01-a-map-on-screen.md) was three lines of colour literals.
That is content as well, so it goes in a file of the same shape:

<!-- include: ../../../examples/tutorial/assets/tiles.ron -->
```ron
// How the warren's tiles look in full light. The renderer works out
// darkness and memory from these two colours.
//
// Every field (the ones marked "optional" may be left out):
//   tile:    the tile's registered name; every registered tile needs a line,
//            and the load says which is missing
//   glyph:   one character
//   fg:      (r, g, b) in 0..=1, the glyph's colour
//   bg:      optional; (r, g, b), the cell's fill; black when left out
//   vary:    optional; (brightness, hue), how far each cell strays from the
//            authored colour, 0..=1 each; none when left out
//   shimmer: optional; brightness drifting over time, 0..=1, for water and
//            anything else that should not sit still
[
    (tile: "earth", glyph: '#', fg: (0.78, 0.66, 0.50), bg: (0.34, 0.27, 0.21), vary: (0.20, 0.05)),
    (tile: "dirt",  glyph: '.', fg: (0.66, 0.58, 0.45), bg: (0.18, 0.15, 0.12), vary: (0.28, 0.06)),
    (tile: "roots", glyph: '+', fg: (0.55, 0.74, 0.45), bg: (0.16, 0.22, 0.13), vary: (0.18, 0.05)),
]
```

<!-- include: ../../../examples/tutorial/src/bin/step08_content.rs:looks -->
```rust,no_run
    /// Both colours of every tile, and how much each cell strays from its
    /// neighbours, read from `assets/tiles.ron` against the tiles registered
    /// above. A tile the file forgets is reported at startup, by name.
    fn appearance(&self) -> TileAppearance {
        TileAppearance::load(TILES_RON, &self.tiles).unwrap_or_else(|e| panic!("assets/tiles.ron: {e}"))
    }
```

The load is checked against the registry the same way the bestiary is checked against the damage kinds.
A tile the file forgets, a name it misspells or a tile it describes twice stops the run at start-up with every problem listed, rather than showing up as a magenta question mark three floors down.
What a tile *is* stays in `Warren::new`, because the engine reads that; what it looks like is the renderer's business and the file's.

## Try it

- Add a monster of your own to `rats.ron`. You will not touch a Rust file.
- Give something `spawn: (1, 4, 20, 6, 10)` and meet a swarm.
- Break the file on purpose, by duplicating a name, writing `"1z6"`, or giving a rat a `kind` nobody registered, and read the error.
- Recolour the roots in `tiles.ron`, then delete the line and read what the load says.

Next: [an action of your own](09-an-action-of-your-own.md).
