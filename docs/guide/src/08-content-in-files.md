# Content in files

> Run it: `cargo run -p tutorial --bin step08_content`
> Source: [`step08_content.rs`](https://github.com/rudehn/rl-engine/blob/main/examples/tutorial/src/bin/step08_content.rs)

Warren has one kind of rat because a `spawn` call was hard-coded.
Move the bestiary into a file and the game stops needing a recompile to gain a monster.

## The file

Every RON schema in this repository lists its full option space at the top, because the file is the interface.

```ron
{{#include ../../../examples/tutorial/assets/rats.ron}}
```

## The struct

```rust,no_run
{{#include ../../../examples/tutorial/src/bin/step08_content.rs:def}}
```

`Named` tells the registry what to key an entry by.
`DiceRoll` deserializes straight from `"1d5+1"`.
`NameRef<DamageKind>` is the interesting one: in the file it is a name, `kind: "venom"`, and by the time the game holds a `RatDef` it is the id of a damage kind.

## Names become ids at load

```rust,no_run
{{#include ../../../examples/tutorial/src/bin/step08_content.rs:bestiary}}
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

## Try it

- Add a monster of your own to `rats.ron`. You will not touch a Rust file.
- Give something `spawn: (1, 4, 20, 6, 10)` and meet a swarm.
- Break the file on purpose, by duplicating a name, writing `"1z6"`, or giving a rat a `kind` nobody registered, and read the error.
