# Things to pick up

> Run it: `cargo run -p tutorial --bin step06_items`
> Source: [`step06_items.rs`](https://github.com/rudehn/rl-engine/blob/main/examples/tutorial/src/bin/step06_items.rs)

`ItemsPlugin` moves items between the ground, a bag and a slot, with five actions: `PickUp`, `DropItem`, `Equip`, `Unequip`, `UseItem`.
It has no opinion about what an item is.

## An item

```rust,no_run
{{#include ../../../examples/tutorial/src/bin/step06_items.rs:crust}}
```

```rust
            commands.spawn((Item, Crust(8), Name::new("a crust of bread"), Position(p), Glyph::new('%', Color::srgb(0.85, 0.72, 0.40)).on_layer(2)));
```

`Item` says the engine may move it.
`Position` means it is on the floor; picking it up removes that component, dropping it puts one back.
`Crust(8)` is yours.

```rust,no_run
{{#include ../../../examples/tutorial/src/bin/step06_items.rs:intents}}
```

Clippy here refuses a system with more than seven parameters, and actions accumulate.
Bundling the writers into a `SystemParam` is the answer; the engine's own systems do the same.

## Using an item

`UseItem` checks the item is in the bag, spends the turn and writes `ItemEvent::Used`.
It does not heal, teleport or explode.

```rust,no_run
{{#include ../../../examples/tutorial/src/bin/step06_items.rs:eat}}
```

Despawning the crust is enough to get it out of the bag.
The engine drops vanished items from every inventory and every slot.

That is the escape hatch, and Warren takes it because it has no abilities yet.
The other way is for the item to `Grants` an ability whose cost is a `Charge`: the engine turns `UseItem` into a `Use` of it, spends the charge from the item, one off a stack or the item itself, and the bag describes the item by what it lends.
A potion is then one line in a content file and no system at all.
Corsair's rum is written that way; [chapter 12](12-where-to-go-next.md) says where to look.

## React, not Narrate

```rust,no_run
        .add_systems(Turn, (populate, eat).in_set(TurnSet::React))
```

`TurnSet::React` runs inside a turn pass, after that pass's actions resolve and before the actor is requeued.

Put `eat` in `PresentSet::Narrate` instead and it works, until a rat kills the player on the same frame the crust was eaten: drawing happens after all the turns in a frame, so the healing landed a blow too late.
Inside the turn, the drink heals before the next blow and a bite poisons on the bite.

`React` runs once per pass and a busy frame runs hundreds, so keep it to reactions.
A system that scans the whole world every frame belongs in `PresentSet::Narrate`.

## Try it

- Give the crust a `Stack { key, count }`, drop six in one cell, pick them all up at once.
- Add a `Wearable` and an equipment slot graph, and make a knife that changes your `MeleeAttack`.
- Move `eat` to `PresentSet::Narrate` and try to catch it healing a turn late.
