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

## Registries validate at load

```rust,no_run
{{#include ../../../examples/tutorial/src/bin/step08_content.rs:bestiary}}
```

`from_ron_str` parses and checks: names unique, every entry well formed.
It fails at start-up naming the file, not three floors down.

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
- Break the file on purpose, by duplicating a name or writing `"1z6"`, and read the error.
