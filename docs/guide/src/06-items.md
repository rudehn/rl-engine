# Things to pick up

> Run it: `cargo run -p tutorial --bin step06_items`
>
> Source: [`step06_items.rs`](https://github.com/rudehn/rl-engine/blob/main/examples/tutorial/src/bin/step06_items.rs)

`ItemsPlugin` moves items between the ground, a bag and a slot, with five actions: `PickUp`, `DropItem`, `Equip`, `Unequip`, `UseItem`.
It has no opinion about what an item is.

## An item

<!-- include: ../../../examples/tutorial/src/bin/step06_items.rs:crust -->
```rust,no_run
/// A crust of bread: the one item the warren has, and how much it heals.
#[derive(Component, Clone, Copy)]
struct Crust(i32);
```

```rust
            commands.spawn((Item, Crust(8), Name::new("a crust of bread"), Position(p), Glyph::new('%', Color::srgb(0.85, 0.72, 0.40)).on_layer(2)));
```

`Item` says the engine may move it.
`Position` means it is on the floor; picking it up removes that component, dropping it puts one back.
`Crust(8)` is yours.

<!-- include: ../../../examples/tutorial/src/bin/step06_items.rs:intents -->
```rust,no_run
/// Everything the player's keys can ask for. A system may take seven
/// parameters; bundling the writers into one `SystemParam` keeps room for
/// as many actions as the game grows.
#[derive(bevy::ecs::system::SystemParam)]
struct PlayerIntents<'w> {
    bumps: MessageWriter<'w, Intent<Bump>>,
    waits: MessageWriter<'w, Intent<Wait>>,
    pick_ups: MessageWriter<'w, Intent<PickUp>>,
    uses: MessageWriter<'w, Intent<UseItem>>,
}
```

Bundling the writers into one `SystemParam` keeps the signature short as the actions accumulate, and the engine's own systems do the same.

## Using an item

`UseItem` checks the item is in the bag, spends the turn and writes `ItemEvent::Used`.
It does not heal, teleport or explode.

<!-- include: ../../../examples/tutorial/src/bin/step06_items.rs:eat -->
```rust,no_run
/// What eating a crust means. The engine has already spent the turn and
/// taken the item out of the bag; this is the part only the game knows.
///
/// It runs in [`TurnSet::React`], inside the turn, so the healing lands
/// before the next rat is dealt its move. In the drawing phase it would
/// land a blow too late.
fn eat(mut commands: Commands, mut used: MessageReader<ItemEvent>, crusts: Query<&Crust>, mut eaters: Query<&mut Health>) {
    for ev in used.read() {
        let ItemEvent::Used { actor, item } = *ev else { continue };
        let (Ok(crust), Ok(mut health)) = (crusts.get(item), eaters.get_mut(actor)) else { continue };
        health.current = (health.current + crust.0).min(health.max);
        commands.entity(item).despawn();
    }
}
```

Despawning the crust is enough to get it out of the bag.
The engine drops vanished items from every inventory and every slot.

Warren takes that escape hatch because it has no abilities yet.
The other way is for the item to `Grants` an ability whose cost is a `Charge`: the engine turns `UseItem` into a `Use` of it, spends the charge from the item, one off a stack or the item itself, and the bag describes the item by what it lends.
A potion is then one line in a content file and no system at all.
Corsair's rum is written that way; [chapter 12](12-where-to-go-next.md) says where to look.

## Where a reaction goes

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

Next: [down the stairs](07-down-the-stairs.md).
