# Work and revival: implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** An actor can spend many turns on one thing, breaking off when hurt, killed or out of reach, and a body can be stood back up; Foundry's repair drone uses both to rebuild droid wrecks, and the player sees `(repairing)` on its row and "Repairing the line droid remains, 4 turns left" in inspect.

**Architecture:** `rl-rules` gains the `Work` model, tested without an `App`. `rl-bevy` gains `WorkPlugin` (start, continue, break, save fields) and, in `remains`, a disabled twin of every actor that leaves remains, taken the moment it dies and despawned the moment its body stops being remains, from which `revive` restores whatever the body is missing. `rl-ui` shows work on the row and in inspect. Foundry adds a repair drone.

**Tech Stack:** Rust 2024, Bevy 0.19 ECS (`EntityCloner`, `Disabled`, component hooks), `serde`/`ron`, mdBook guide pages.

**Spec:** `docs/design/work.md` and `docs/design/remains.md` §9. Read both before starting; this plan argues from them, and `work.md` §10 lists what is deliberately not built.

## Global Constraints

- Never an em dash anywhere, code or prose; use a plain dash.
- Commit messages never carry a `Co-Authored-By` line naming an agent (the user's rule overrides the harness's reminder).
- Commit messages follow the repo's style: a lower-case sentence saying what is now true, as in `git log --oneline -10`.
- Work on a branch named `work-and-revival` in a worktree (superpowers:using-git-worktrees).
- Each task's commit passes the fast gate: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test -p <each crate the task touched>`. Each commit writes its own `CHANGELOG.md` line under `Unreleased` when a game on the previous release would change a line or want to; each task says whether it owes one.
- The branch pays its documentation once, in Task 10, which also runs everything CI runs: `cargo test --workspace`, `scripts/check-tiers.sh`, `scripts/check-tiers.sh --wasm`, `scripts/check-overview.sh`, `python3 scripts/check-systems.py`, `scripts/check-guide.sh`.
- `rl-core`, `rl-grid` and `rl-rules` are tier 0 and 1: no Bevy, they build on `wasm32-unknown-unknown`, no `std::time::Instant`.
- `#![deny(missing_docs)]`: every public item gets a doc comment that says why and why-not, at the density of `crates/rl-core/src/turn.rs`, not more. Module-level `//!` docs name every public item.
- No `HashMap`/`HashSet` on gameplay paths; `Vec` or `BTreeMap`. No `TODO` comments in source.
- Clocks and costs are integers in hundredths of a step. Work counts turns, never clock time.
- No theme words in engine crates: the engine says `work`, `remains`, `revive`; `repair`, `drone` and `wreck` are Foundry's. Engine test fixtures use neutral words ("mending").
- Never order a system after another crate's system function. Within `rl-bevy`, `.after(crate::combat::apply_damage)` is allowed, as `leave_remains` already does with `process_deaths`.
- Every RON schema carries a top-of-file comment listing the full option space; a new field is added to it in the same commit.
- Long Markdown: one sentence per physical line.
- The numbers: reach is one cell, Chebyshev. Foundry's repair drone is `(name: "repair drone", glyph: 'u', color: (0.55, 0.8, 0.75), hp: 6, armor: 0, profile: "chassis", faction: "droids", wits: ["mindless", "opens_doors"], perception: 8, dark_sight: 4, speed: 100, flee_at: 50, hearing: (threshold: 0, memory: 8), drops: [("slug", 10)], repairs: 4)`, spawned on decks 3 to 10 at weight 25 in groups of one. A wreck takes `repairs` turns plus one for every two points of its kind's health (line droid 8, probe 7, trooper 9, heavy 13), and stands up with half its maximum health rounded up. These are proposals; the user may change them at plan review.

## Review Focus

The inputs most likely to bite a player that no happy-path test exercises; each has its test in the owning task.

1. **A body revived where someone now stands**: the repair drone may stand on the wreck it rebuilds, so the droid must stand up on the nearest free cell, never on top of it (Task 3).
2. **A looted wreck**: it comes back without what was taken, worn or carried, and no item ends up in two bags (Tasks 1 and 3).
3. **A killing blow on a worker**: it breaks as `Died`, not `Hurt`, and the actor revived from it comes back still working, breaking at once as `OutOfReach` if its target has gone (Task 6).
4. **Work of one turn**: `needed` of one finishes on the turn it is begun and never leaves a `Working` behind (Task 5).
5. **A continued run**: a body from a save can be revived, and work comes back where it was, or not at all when its target was not saved (Tasks 2, 3 and 7).

---

## File map

| File | Change | Responsibility |
|---|---|---|
| `Cargo.toml` | modify | Bevy's `debug` feature, so a component that cannot be copied is named |
| `crates/rl-bevy/src/props.rs` | modify | taking a worn item out of a container unequips it |
| `crates/rl-bevy/src/remains.rs` | modify | `Life`, `keep_life`, `take_twin`, `uncopied`, `lay_down`, `revive`, `ReviveCommands`, `Revived` |
| `crates/rl-rules/src/work.rs` | create | `Work`, `Progress`, `WorkKind`, `WorkKindId`, `REACH`, `in_reach` |
| `crates/rl-rules/src/ai/brain.rs` | modify | `Decision::Work` |
| `crates/rl-bevy/src/work.rs` | create | `WorkPlugin`, `WorkKinds`, `AddWork`, `Working`, `BeginWork`, `Toil`, `Works`, `WorkBegan`, `WorkDone`, `WorkBroken`, `BreakReason`, the systems |
| `crates/rl-bevy/src/minds.rs` | modify | a mind's `Decision::Work` becomes `Intent<BeginWork>` |
| `crates/rl-bevy/src/lib.rs` | modify | `pub mod work;` and exports |
| `crates/rl-save/src/run.rs` | modify | lay-down through `lay_down`; `EntityState::working` |
| `crates/rl-ui/src/view/mod.rs` | modify | `WorkRow`, `Row::work`, `Workings` |
| `crates/rl-ui/src/view/nearby.rs`, `panel/nearby.rs` | modify | the row's work word |
| `crates/rl-ui/src/view/inspect.rs`, `panel/inspect.rs` | modify | the inspect line and its templates |
| `examples/foundry/src/droids/repair.rs` | create | `Wrecks`, `sense_wrecks`, `RepairWrecks`, `rebuild_wrecks` |
| `examples/foundry/src/droids.rs`, `plugin.rs`, `assets/monsters.ron`, `assets/monster_spawns.ron` | modify | the drone |

---

## Task 1: A worn thing taken from a container is no longer worn by it

**Files:**
- Modify: `crates/rl-bevy/src/props.rs` (`resolve_takes`, near line 475)
- Test: `crates/rl-bevy/src/props.rs` tests module, beside `taking_moves_things_into_the_bag_and_taking_all_costs_one_turn`
- Docs: `CHANGELOG.md`

**Interfaces:**
- Produces: after a `Take`, no taken item is listed in the container's `Equipped`. Task 3's rule for items rests on it.

The user's rule is to reproduce a bug before fixing it. This one has no symptom a player sees today, since nothing reads a body's `Equipped`; the failing test is the reproduction, and Task 10 checks a looted wreck in the running game.

- [ ] **Step 1: Write the failing test**

```rust
    /// A worn thing taken out of a body comes off it: what a body wears
    /// is what it still carries, so nothing looted is still worn by what
    /// it was looted from, and nothing brought back to life wears it.
    #[test]
    fn a_worn_thing_taken_from_a_container_is_no_longer_worn_by_it() {
        let mut it = chest("supply crate");
        let first = bag_of(&it.app, it.prop)[0];
        let mut worn = rl_rules::Equipment::with_slot_count(1);
        worn.equip(first, &rl_rules::EquipShape::in_slot(rl_rules::SlotId::from_raw(0))).expect("one empty slot");
        it.app.world_mut().entity_mut(it.prop).insert(crate::items::Equipped(worn));
        it.app.world_mut().write_message(Intent::new(it.player, Take { from: it.prop, item: Some(first) }));
        it.app.update();
        assert_eq!(bag_of(&it.app, it.player), vec![first], "taken");
        let still = it.app.world().get::<crate::items::Equipped>(it.prop).expect("the container still has its slots");
        assert!(!still.contains(first), "and no longer worn by what it was taken from");
    }
```

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test -p rl-bevy a_worn_thing_taken_from_a_container_is_no_longer_worn_by_it`
Expected: FAIL at "and no longer worn by what it was taken from".

- [ ] **Step 3: Unequip what is taken**

Add a parameter to `resolve_takes`:

```rust
    mut worn: Query<&mut crate::items::Equipped>,
```

and after `contents.items.retain(|i| !taking.contains(i));`:

```rust
        // What a thing wears is what it still carries: a worn thing taken
        // out of a body comes off it, or the body goes on listing armor
        // that is now in someone's bag, and a body stood back up wears it.
        if let Ok(mut worn) = worn.get_mut(intent.action.from) {
            for item in &taking {
                worn.unequip(*item);
            }
        }
```

- [ ] **Step 4: Run the test and the crate**

Run: `cargo test -p rl-bevy`
Expected: PASS.

- [ ] **Step 5: Changelog and commit**

```markdown
- Taking a worn thing out of a container takes it out of the container's `Equipped` as well as its bag. A looted body went on listing the armor it wore after the armor was in the player's bag.
```

```bash
git add crates/rl-bevy/src/props.rs CHANGELOG.md
git commit -m "a worn thing taken out of a container is no longer worn by it"
```

---

## Task 2: The living actor is kept on a twin, for as long as its body is remains

**Files:**
- Modify: `Cargo.toml` (the `bevy` features list), `crates/rl-bevy/src/remains.rs`, `crates/rl-bevy/src/lib.rs`, `crates/rl-save/src/run.rs` (the lay-down near line 490)
- Test: `crates/rl-bevy/src/remains.rs` tests, `crates/rl-save/src/run.rs` tests
- Docs: `CHANGELOG.md`

**Interfaces:**
- Produces: `rl_bevy::remains::Life(Entity)`, a component on a body linking its twin; `pub fn take_twin(world: &mut World, entity: Entity)`; `pub fn uncopied(world: &World, from: Entity, to: Entity) -> Vec<String>`; `pub fn lay_down(world: &mut World, entity: Entity, since: u32, credit: Option<Entity>)`; the system `keep_life`. Task 3 reads the twin through `Life`.

- [ ] **Step 1: Turn on Bevy's `debug` feature**

In the workspace `Cargo.toml`, add `"debug",` to the `bevy` features list after `"std",`.
Without it, `ComponentInfo::name` reads "<Enable the debug feature to see the name>" and the report in Step 5 could not say which component to fix. Run `cargo build -p rl-bevy` and `scripts/check-tiers.sh --wasm` to confirm both still build.

- [ ] **Step 2: Write the failing tests**

In the tests module of `crates/rl-bevy/src/remains.rs`, add these imports and fixtures:

```rust
    use bevy::ecs::entity_disabling::Disabled;

    /// A game's own component, which the game's own death takes off.
    #[derive(Component, Debug, Clone, PartialEq)]
    struct Patrol(i32);

    /// The game's answer to a death: its patrol ends.
    fn end_patrols(mut commands: Commands, mut deaths: MessageReader<DeathEvent>) {
        for death in deaths.read() {
            commands.entity(death.entity).remove::<Patrol>();
        }
    }

    /// A component a game forgot to make `Clone`.
    #[derive(Component)]
    struct Unclonable;

    /// Every twin in the world, which a query sees only by naming `Disabled`.
    fn twins(app: &mut App) -> usize {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<Disabled>>().iter(world).count()
    }
```

and the tests:

```rust
    /// The copy is taken the moment the actor dies, before anything takes
    /// anything off it: a component the game's own death system removes is
    /// on the twin, and no query that does not ask for disabled entities
    /// ever sees the twin.
    #[test]
    fn a_dying_actor_is_kept_whole_on_a_twin_no_query_sees() {
        let (mut app, start, sides) = arena();
        app.add_systems(crate::plugin::Turn, end_patrols.in_set(crate::plugin::TurnSet::React));
        let dead = victim(&mut app, start.offset(2, 0), sides, true);
        app.world_mut().entity_mut(dead).insert(Patrol(3));
        kill(&mut app, dead, sides);

        let twin = app.world().get::<Life>(dead).expect("the body keeps a life to return to").0;
        let world = app.world();
        assert_eq!(world.get::<Patrol>(twin), Some(&Patrol(3)), "what the game's death took off is on the twin");
        assert!(world.get::<Patrol>(dead).is_none(), "and off the body");
        assert!(world.get::<Actor>(twin).is_some() && world.get::<Health>(twin).is_some(), "the twin is the actor as it lived");
        let world = app.world_mut();
        let actors: Vec<Entity> = world.query_filtered::<Entity, With<Actor>>().iter(world).collect();
        assert!(!actors.contains(&twin), "no ordinary query sees the twin");
    }

    /// The twin lives exactly as long as its body is remains: despawning
    /// the body or taking `Remains` off it takes the twin too.
    #[test]
    fn no_twin_outlives_its_body_however_the_body_stops_being_remains() {
        let (mut app, start, sides) = arena();
        let despawned = victim(&mut app, start.offset(2, 0), sides, true);
        let stripped = victim(&mut app, start.offset(3, 0), sides, true);
        kill(&mut app, despawned, sides);
        kill(&mut app, stripped, sides);
        assert_eq!(twins(&mut app), 2, "one twin a body");

        app.world_mut().despawn(despawned);
        app.update();
        assert_eq!(twins(&mut app), 1, "a body despawned takes its twin");

        app.world_mut().entity_mut(stripped).remove::<Remains>();
        app.update();
        assert_eq!(twins(&mut app), 0, "and one that stops being remains does too");
        assert!(app.world().get::<Life>(stripped).is_none());
    }

    /// Bevy copies only what is `Clone` and skips the rest without a word,
    /// so the engine compares the two and names what did not come across.
    #[test]
    fn a_component_that_cannot_be_copied_is_named() {
        let (mut app, start, sides) = arena();
        let dead = victim(&mut app, start.offset(2, 0), sides, true);
        app.world_mut().entity_mut(dead).insert(Unclonable);
        kill(&mut app, dead, sides);
        let twin = app.world().get::<Life>(dead).unwrap().0;
        let lost = uncopied(app.world(), dead, twin);
        assert!(lost.iter().any(|n| n.contains("Unclonable")), "{lost:?}");
        assert!(!lost.iter().any(|n| n.contains("Health")), "and nothing that was copied: {lost:?}");
    }
```

In `crates/rl-save/src/run.rs` tests, extend `remains_are_still_remains_when_the_run_is_continued` with, after its last assertion:

```rust
        assert!(w.get::<rl_bevy::remains::Life>(ada2).is_some(), "and she keeps a life to return to, taken as she was laid down again");
```

- [ ] **Step 3: Run them to see them fail**

Run: `cargo test -p rl-bevy remains` and `cargo test -p rl-save remains_are_still_remains`
Expected: FAIL to compile, `Life`, `uncopied` not found.

- [ ] **Step 4: `Life`, its hooks, and the hook on `Remains`**

In `crates/rl-bevy/src/remains.rs`, add imports:

```rust
use bevy::ecs::component::ComponentId;
use bevy::ecs::entity::EntityCloner;
use bevy::ecs::entity_disabling::Disabled;
use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::world::DeferredWorld;
```

Give `Remains` a hook, keeping its derive:

```rust
#[derive(Component, Debug, Clone, Copy)]
#[component(on_remove = end_life)]
pub struct Remains {
```

and add:

```rust
/// The living actor as it was the moment it died, kept on a twin no
/// query sees, for [`revive`] to stand it back up from.
///
/// On the body rather than in [`Remains`], because the copy is taken the
/// pass the actor dies, before it is laid down, and a body despawned in
/// between must still take its twin with it. Its hook is that guarantee:
/// whatever takes this off the body, or despawns the body, despawns the
/// twin, so no path through the engine or a game leaves one behind.
#[derive(Component, Debug, Clone, Copy)]
#[component(on_remove = despawn_twin)]
pub struct Life(pub Entity);

fn despawn_twin(mut world: DeferredWorld, ctx: HookContext) {
    if let Some(twin) = world.get::<Life>(ctx.entity).map(|l| l.0) {
        world.commands().entity(twin).try_despawn();
    }
}

/// A body that stops being remains, however, stops keeping a life to
/// return to.
fn end_life(mut world: DeferredWorld, ctx: HookContext) {
    world.commands().entity(ctx.entity).try_remove::<Life>();
}
```

- [ ] **Step 5: Taking the twin, and naming what it could not take**

```rust
/// What `from` has that `to` does not, by name: the components a copy
/// from one to the other could not carry.
///
/// Bevy copies a component only if it is `Clone` and skips one that is
/// not without a word, which would leave a revived actor quietly short of
/// something. Named rather than counted, so the report says what to fix.
pub fn uncopied(world: &World, from: Entity, to: Entity) -> Vec<String> {
    let copied: Vec<ComponentId> = world.entity(to).archetype().components().to_vec();
    world
        .entity(from)
        .archetype()
        .components()
        .iter()
        .filter(|c| !copied.contains(c))
        .filter_map(|c| world.components().get_name(*c))
        .map(|name| name.to_string())
        .collect()
}

/// Copies `entity`, as it is this moment, onto a twin that is `Disabled`,
/// and links it with [`Life`].
///
/// Every component is copied and none is named, so one added to the
/// engine or to a game next year comes back from a revival without anyone
/// remembering to list it. A second call on a body that already has a twin
/// does nothing: a death takes one, and laying the body down afterwards
/// must not take a second of what is by then a body.
pub fn take_twin(world: &mut World, entity: Entity) {
    if world.get_entity(entity).is_err() || world.get::<Life>(entity).is_some() {
        return;
    }
    let twin = world.spawn(Disabled).id();
    EntityCloner::build_opt_out(world).clone_entity(entity, twin);
    let lost = uncopied(world, entity, twin);
    if !lost.is_empty() {
        error!("{} cannot come back to life: derive `Clone` on it", lost.join(", "));
    }
    world.entity_mut(entity).insert(Life(twin));
}

/// Keeps a twin of every actor that will leave remains, the moment it dies.
///
/// In `ResolveSet::Damage` straight after `apply_damage`, which is where
/// the death is written: before `TurnSet::React`, where a game answers a
/// death and may take its own components off, and before
/// `CleanupSet::Remove`, where the engine takes the actor out of the world.
/// The same test as [`leave_remains`] decides who, so every body has a
/// twin and nothing else does.
pub fn keep_life(mut commands: Commands, mut deaths: MessageReader<DeathEvent>, leaves: Query<(), With<LeavesRemains>>) {
    for death in deaths.read() {
        if death.was_player || leaves.get(death.entity).is_err() {
            continue;
        }
        let entity = death.entity;
        commands.queue(move |world: &mut World| take_twin(world, entity));
    }
}
```

- [ ] **Step 6: One lay-down, for a death and for a save**

```rust
/// Lays `entity` down as remains: takes a twin if it has none, takes the
/// life off, makes it a prop and names it as what is left of it.
///
/// One function, used by a death and by a save continued, so a body from
/// a save has a twin as surely as one that just fell. On the second path
/// the twin is the living thing the game's own record just spawned, which
/// is all a save knows.
pub fn lay_down(world: &mut World, entity: Entity, since: u32, credit: Option<Entity>) {
    take_twin(world, entity);
    let Ok(mut body) = world.get_entity_mut(entity) else { return };
    body.remove::<WasLiving>().insert((crate::props::Prop, Remains { since, credit }));
    name_as_remains(world, entity);
}
```

Rewrite the loop body of `leave_remains` to:

```rust
        // A body is a prop: something standing in a cell that is neither an
        // actor nor an item, which is what lets a mind walk to one, a game
        // offer a verb on one, and every panel list one, with no second
        // mechanism for bodies. It lies where it fell, which death took off.
        commands.entity(death.entity).insert(Position(death.at));
        let (entity, since, credit) = (death.entity, turns.now(), death.credit);
        commands.queue(move |world: &mut World| lay_down(world, entity, since, credit));
        left.write(RemainsLeft { entity: death.entity, at: death.at });
```

Register the system in `RemainsPlugin::build`:

```rust
            .add_systems(Turn, keep_life.in_set(crate::plugin::ResolveSet::Damage).after(crate::combat::apply_damage))
```

Update the module `//!` docs: replace "Nothing is copied and nothing is spawned" with a sentence saying the actor is kept as it is, and a twin of it as it lived is kept beside it for [`revive`] until the body stops being remains; name `Life`, `take_twin`, `uncopied`, `lay_down` and `keep_life`. Replace "The engine never removes remains." with "The engine never removes remains of its own accord."

In `crates/rl-bevy/src/lib.rs`, extend the `remains` re-export to include `Life`.

- [ ] **Step 7: The save lays down through the same function**

In `crates/rl-save/src/run.rs`, replace

```rust
            if let Ok(mut target) = world.get_entity_mut(entity) {
                target.remove::<WasLiving>().insert((rl_bevy::Prop, Remains { since, credit }));
            }
            // And named as what is left of what it was, from the one place
            // the wording lives: a game's record says what it was.
            rl_bevy::remains::name_as_remains(world, entity);
```

with

```rust
            // The one lay-down a death also goes through, so the body keeps
            // a twin to be stood back up from, and is named as what is left
            // of what it was from the one place the wording lives.
            rl_bevy::remains::lay_down(world, entity, since, credit);
```

and drop `WasLiving` from the import list if nothing else uses it.

- [ ] **Step 8: Run the tests**

Run: `cargo test -p rl-bevy remains`, `cargo test -p rl-save`, then `cargo test -p rl-bevy`
Expected: PASS.

- [ ] **Step 9: Changelog and commit**

```markdown
- A body keeps a twin of the actor it was: the moment an actor that `LeavesRemains` dies, `RemainsPlugin` copies it whole onto an entity that is `Disabled`, which no ordinary query sees, and despawns that copy the moment the body stops being remains, whether it is despawned or has `Remains` taken off. A component that is not `Clone` cannot be copied, and is named in an error when the actor dies. `remains::lay_down` is the one lay-down a death and a continued save go through. The workspace turns on Bevy's `debug` feature, so component names read as their types.
```

```bash
git add Cargo.toml Cargo.lock crates/rl-bevy/src/remains.rs crates/rl-bevy/src/lib.rs crates/rl-save/src/run.rs CHANGELOG.md
git commit -m "a body keeps a twin of the actor it was, for exactly as long as it is remains"
```

---

## Task 3: A body can be stood back up

**Files:**
- Modify: `crates/rl-bevy/src/remains.rs`, `crates/rl-bevy/src/lib.rs`
- Test: `crates/rl-bevy/src/remains.rs` tests, `crates/rl-save/src/run.rs` tests
- Docs: `CHANGELOG.md`

**Interfaces:**
- Consumes: `Life`, `lay_down` (Task 2).
- Produces: `pub fn revive(world: &mut World, body: Entity, health: i32) -> bool`; `pub trait ReviveCommands { fn revive(&mut self, body: Entity, health: i32); }` implemented for `Commands`; `#[derive(Message)] pub struct Revived { pub entity: Entity }`; `pub const STANDING_ROOM: i32 = 2`. Task 9 calls `commands.revive(wreck, health)`.

- [ ] **Step 1: Write the failing tests**

Add to the `crates/rl-bevy/src/remains.rs` tests module:

```rust
    use crate::items::{Equipped, Inventory, Item};

    /// Stands `body` up with `health`, the way a game does, and runs the
    /// frame that admits it.
    fn stand_up(app: &mut App, body: Entity, health: i32) -> bool {
        let stood = revive(app.world_mut(), body, health);
        app.update();
        stood
    }

    /// A revived actor has exactly the components it had when it died,
    /// less none and plus none, whoever took what off in between.
    #[test]
    fn a_revived_actor_has_exactly_what_it_had_when_it_died() {
        let (mut app, start, sides) = arena();
        app.add_systems(crate::plugin::Turn, end_patrols.in_set(crate::plugin::TurnSet::React));
        let dead = victim(&mut app, start.offset(2, 0), sides, true);
        app.world_mut().entity_mut(dead).insert((Patrol(3), Name::new("line droid")));
        // Playing, so everything play puts on an actor (its `OnMap`) is on
        // it before the set is taken.
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        let turn_state = app.world().components().component_id::<crate::components::MyTurn>();
        let set = |app: &App| -> Vec<ComponentId> {
            let mut ids: Vec<ComponentId> =
                app.world().entity(dead).archetype().components().iter().copied().filter(|c| Some(*c) != turn_state).collect();
            ids.sort();
            ids
        };
        let before = set(&app);
        kill(&mut app, dead, sides);
        assert!(stand_up(&mut app, dead, 3));

        assert_eq!(set(&app), before, "the same components it had alive");
        let world = app.world();
        assert_eq!(world.get::<Patrol>(dead), Some(&Patrol(3)), "the game's own, which its death took off, is back");
        assert_eq!(world.get::<Name>(dead).map(|n| n.as_str().to_string()), Some("line droid".into()), "and its own name, not a body's");
        assert_eq!(world.get::<Health>(dead).map(|h| (h.current, h.max)), Some((3, 4)), "with the health it was given");
        assert_eq!(twins(&mut app), 0, "and no twin left behind");
    }

    /// Stood up, it is an actor again in every way that matters: dealt
    /// turns, in the way, able to be hurt, and able to die and leave
    /// remains a second time.
    #[test]
    fn a_revived_actor_is_dealt_turns_blocks_and_can_die_again_leaving_remains_again() {
        let (mut app, start, sides) = arena();
        let dead = victim(&mut app, start.offset(2, 0), sides, true);
        kill(&mut app, dead, sides);
        assert!(stand_up(&mut app, dead, 2));
        app.update();
        assert!(app.world().resource::<crate::turn::Turns>().contains(dead), "back in the queue");
        assert!(app.world().resource::<crate::turn::Occupancy>().is_occupied(start.offset(2, 0)), "and in the way");
        assert!(app.world().get::<Remains>(dead).is_none() && app.world().get::<crate::props::Prop>(dead).is_none(), "and no longer a body");

        kill(&mut app, dead, sides);
        assert!(app.world().get::<Remains>(dead).is_some(), "it died again, and lies there again");
        assert_eq!(twins(&mut app), 1, "with a twin of its second life");
    }

    /// A looted body comes back without what was taken, worn or carried;
    /// one whose bag was taken away comes back with none; and a bag it
    /// gained as a body is left on the floor rather than lost.
    #[test]
    fn a_revived_body_carries_only_what_it_still_holds_and_no_item_is_in_two_bags() {
        let (mut app, start, sides) = arena();
        let kept = app.world_mut().spawn(Item).id();
        let looted = app.world_mut().spawn(Item).id();
        let dead = victim(&mut app, start.offset(2, 0), sides, true);
        let mut worn = rl_rules::Equipment::with_slot_count(1);
        worn.equip(looted, &rl_rules::EquipShape::in_slot(rl_rules::SlotId::from_raw(0))).unwrap();
        app.world_mut().entity_mut(dead).insert((Inventory { items: vec![kept, looted] }, Equipped(worn)));
        kill(&mut app, dead, sides);
        // Looting, as `resolve_takes` does it.
        app.world_mut().get_mut::<Inventory>(dead).unwrap().remove(looted);
        app.world_mut().get_mut::<Equipped>(dead).unwrap().unequip(looted);
        assert!(stand_up(&mut app, dead, 2));
        assert_eq!(app.world().get::<Inventory>(dead).map(|b| b.items.clone()), Some(vec![kept]), "only what it still held");
        assert!(!app.world().get::<Equipped>(dead).unwrap().contains(looted), "and it wears nothing it lost");

        let bagless = victim(&mut app, start.offset(3, 0), sides, true);
        app.world_mut().entity_mut(bagless).insert(Inventory { items: vec![] });
        kill(&mut app, bagless, sides);
        app.world_mut().entity_mut(bagless).remove::<Inventory>();
        assert!(stand_up(&mut app, bagless, 2));
        assert!(app.world().get::<Inventory>(bagless).is_none(), "a bag taken away is not handed back from the twin");

        let gained = victim(&mut app, start.offset(4, 0), sides, true);
        kill(&mut app, gained, sides);
        let loot = app.world_mut().spawn(Item).id();
        app.world_mut().entity_mut(gained).insert(Inventory { items: vec![loot] });
        assert!(stand_up(&mut app, gained, 2));
        assert!(app.world().get::<Inventory>(gained).is_none(), "a bag gained as a body goes");
        assert_eq!(app.world().get::<Position>(loot).map(|p| p.0), Some(start.offset(4, 0)), "and what was in it is on the floor where it stood");
    }

    /// Something standing on a body when it stands up is not stood on: the
    /// body takes the nearest free cell.
    #[test]
    fn a_body_revived_where_someone_stands_stands_up_beside_them() {
        let (mut app, start, sides) = arena();
        let at = start.offset(3, 0);
        let dead = victim(&mut app, at, sides, true);
        kill(&mut app, dead, sides);
        let blocker = app.world_mut().spawn((Actor, Blocks, Position(at), Health::full(4), Faction(sides.ours))).id();
        app.update();
        assert!(stand_up(&mut app, dead, 2));
        let stood = app.world().get::<Position>(dead).unwrap().0;
        assert_ne!(stood, at, "not on top of whoever was there");
        assert!(rl_core::geometry::is_adjacent(stood, at), "but beside them");
        assert_eq!(app.world().get::<Position>(blocker).unwrap().0, at, "who did not move");
    }

    /// Only remains can be stood up.
    #[test]
    fn nothing_but_remains_can_be_revived() {
        let (mut app, start, sides) = arena();
        let alive = victim(&mut app, start.offset(2, 0), sides, true);
        assert!(!revive(app.world_mut(), alive, 3), "a living actor has no life to return to");
    }
```

In `crates/rl-save/src/run.rs` tests, add:

```rust
    /// A body continued from a save stands up as surely as one that just
    /// fell, from the living thing the game's own record respawned.
    #[test]
    fn a_body_continued_from_a_save_can_be_revived() {
        let backend = std::sync::Arc::new(MemoryBackend::default());
        let (mut app, start) = game(Saves(backend.clone()));
        let me = app.world_mut().spawn((Actor, Player, Blocks, You, Position(start), Viewshed::new(6), RevealsMap, Health::full(30))).id();
        let ada = app.world_mut().spawn((Actor, Blocks, Person("Ada".into()), Position(start.offset(0, 3)), Health::full(20), LeavesRemains)).id();
        play(&mut app);
        let kind = app.world().resource::<Registries>().damage_kinds.expect("kinetic");
        app.world_mut().write_message(DamageEvent::new(ada, rl_rules::Hit::by(me, kind, 99)));
        app.update();
        app.update();
        save_run(app.world_mut()).unwrap();

        let (mut back, _) = game(Saves(backend));
        load_run(back.world()).unwrap().expect("a save").restore(back.world_mut()).unwrap();
        play(&mut back);
        let w = back.world_mut();
        let ada2 = w.query_filtered::<Entity, With<Person>>().single(w).expect("Ada came back");
        assert!(rl_bevy::remains::revive(back.world_mut(), ada2, 10), "and can be stood up");
        back.update();
        assert_eq!(back.world().get::<Health>(ada2).map(|h| h.current), Some(10));
        assert!(back.world().get::<Actor>(ada2).is_some(), "an actor again");
    }
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p rl-bevy remains`
Expected: FAIL to compile, `revive` not found.

- [ ] **Step 3: `Revived`, `revive`, and the command**

Add to `crates/rl-bevy/src/remains.rs` (imports: `crate::components::{MyTurn, OnMap, Viewshed}`, `crate::items::{Equipped, Inventory}`, `crate::turn::Occupancy`, `crate::world::WorldMap`, `rl_core::MapId`):

```rust
/// A body was stood back up, this pass, by [`revive`].
///
/// For a game to say so, or to put back what it wants a revived actor to
/// have forgotten; the engine itself restores the actor as it died.
#[derive(Message, Debug, Clone, Copy)]
pub struct Revived {
    /// The actor, which is the entity that was the body.
    pub entity: Entity,
}

/// How far from its body a revived actor may stand when something stands
/// on the body: far enough to find room in a crowd, near enough that it
/// is plainly the same one getting up.
pub const STANDING_ROOM: i32 = 2;

/// Stands `body` back up with `health`, and says whether it could.
///
/// The body is given back the shape of its twin (see [`Life`]): every
/// component the twin has and the body lacks is put back, which is
/// everything death took off, named nowhere; every component the body
/// gained as a body is taken off, `Remains` and `Prop` and whatever the
/// game added; and a component on both keeps the body's value, because
/// that is what has happened since. `Name` is the exception, because the
/// engine itself renamed the body. What it carries is never taken from
/// the twin, whose bag lists items that may since be anywhere: a looted
/// body comes back without what was taken, one whose bag was taken away
/// comes back with none, and a bag it gained as a body is emptied onto the
/// floor before it goes. `MyTurn` is never put back either; the turn
/// queue deals it turns again when it is admitted.
///
/// Refused, with a warning, for anything that is not remains with a twin,
/// and for a body with someone on it and no free cell within
/// [`STANDING_ROOM`].
pub fn revive(world: &mut World, body: Entity, health: i32) -> bool {
    let Some(twin) = world.get::<Life>(body).map(|l| l.0) else {
        warn!("{body:?} cannot be revived: it is not remains, or `RemainsPlugin` kept no twin of it");
        return false;
    };
    let Some(at) = standing_room(world, body) else {
        warn!("{body:?} cannot be revived: something stands on it and nothing within {STANDING_ROOM} cells is free");
        return false;
    };
    let never =
        [world.component_id::<Disabled>(), world.component_id::<Inventory>(), world.component_id::<Equipped>(), world.component_id::<MyTurn>()];
    let body_has: Vec<ComponentId> = world.entity(body).archetype().components().to_vec();
    let twin_has: Vec<ComponentId> = world.entity(twin).archetype().components().to_vec();
    let missing: Vec<ComponentId> = twin_has.iter().copied().filter(|c| !body_has.contains(c) && !never.contains(&Some(*c))).collect();
    let gained: Vec<ComponentId> = body_has.iter().copied().filter(|c| !twin_has.contains(c)).collect();
    if world.component_id::<Inventory>().is_some_and(|bag| gained.contains(&bag)) {
        spill(world, body, at);
    }
    EntityCloner::build_opt_in(world).allow_by_ids(missing).clone_entity(twin, body);
    let name = world.get::<Name>(twin).cloned();
    let max = world.get::<Health>(twin).map_or(health.max(1), |h| h.max);
    let mut stood = world.entity_mut(body);
    stood.remove_by_ids(&gained).insert((Health { current: health.clamp(1, max), max }, Position(at)));
    if let Some(name) = name {
        stood.insert(name);
    }
    if let Some(mut sight) = stood.get_mut::<Viewshed>() {
        sight.dirty = true;
    }
    world.write_message(Revived { entity: body });
    true
}

/// Where a revived body stands: where it lies, or, when something now
/// stands there, the nearest free cell, ring by ring in reading order so
/// the same world always gives the same cell.
fn standing_room(world: &World, body: Entity) -> Option<Point> {
    let at = world.get::<Position>(body)?.0;
    let occupancy = world.resource::<Occupancy>();
    let on = world.get::<OnMap>(body).map(|m| m.0).unwrap_or(MapId::SURFACE);
    if on != occupancy.current() || !occupancy.is_occupied(at) {
        return Some(at);
    }
    let map = world.resource::<WorldMap>();
    (1..=STANDING_ROOM).find_map(|r| {
        (-r..=r).flat_map(|dy| (-r..=r).map(move |dx| (dx, dy))).filter(|(dx, dy)| dx.abs().max(dy.abs()) == r).map(|(dx, dy)| at.offset(dx, dy)).find(|p| map.is_walkable(*p) && !occupancy.is_occupied(*p))
    })
}

/// Empties a bag the body gained as a body onto the floor at `at`, so a
/// revival never destroys an item.
fn spill(world: &mut World, body: Entity, at: Point) {
    let items = world.get::<Inventory>(body).map(|b| b.items.clone()).unwrap_or_default();
    let map = world.get::<OnMap>(body).map(|m| m.0).unwrap_or(MapId::SURFACE);
    for item in items {
        if let Ok(mut thing) = world.get_entity_mut(item) {
            thing.insert((Position(at), OnMap(map)));
        }
    }
}

/// Stands a body up from inside a system, at the next sync point.
pub trait ReviveCommands {
    /// Queues [`revive`] of `body` with `health`.
    fn revive(&mut self, body: Entity, health: i32);
}

impl ReviveCommands for Commands<'_, '_> {
    fn revive(&mut self, body: Entity, health: i32) {
        self.queue(move |world: &mut World| {
            revive(world, body, health);
        });
    }
}
```

Register the message in `RemainsPlugin::build`: `.add_message::<Revived>()`.
If `world.component_id` is not on `World` in this Bevy, use `world.components().component_id::<T>()`; if `Occupancy::current` or `Viewshed::dirty` is private, use the public accessor next to it.
Name `Revived`, `revive`, `ReviveCommands` and `STANDING_ROOM` in the module `//!` docs, and add `Revived, ReviveCommands, revive` to the `remains` re-exports in `lib.rs` and its prelude.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p rl-bevy remains`, `cargo test -p rl-save`, `cargo test -p rl-bevy`
Expected: PASS. If the component-set comparison fails, the assertion's diff names which component came back or did not; fix `revive`'s rule, never the test.

- [ ] **Step 5: Changelog and commit**

```markdown
- A body can be stood back up: `remains::revive(world, body, health)`, or `commands.revive(body, health)` through `ReviveCommands`, gives it back every component its twin has that it lacks, takes off everything it gained as a body, `Remains` and `Prop` and whatever the game added, and writes `Revived`. It comes back with its own name, the health it was given, and what it still carries: never a looted item, never a bag that was taken away, and a bag it gained as a body is left on the floor. Something standing on the body moves it to the nearest free cell within `STANDING_ROOM`.
```

```bash
git add crates/rl-bevy/src/remains.rs crates/rl-bevy/src/lib.rs crates/rl-save/src/run.rs CHANGELOG.md
git commit -m "a body can be stood back up as the actor it was, carrying only what it still holds"
```

---

## Task 4: The work model

**Files:**
- Create: `crates/rl-rules/src/work.rs`
- Modify: `crates/rl-rules/src/lib.rs`
- Test: `crates/rl-rules/src/work.rs`

**Interfaces:**
- Produces: `rl_rules::work::{Work<A>, Progress, WorkKind, WorkKindId, REACH, in_reach}`; `Work::new(kind: WorkKindId, needed: u16) -> Self`, `.on(target: A) -> Self`, `.left() -> u16`, `.advance() -> Progress`; fields `kind`, `target: Option<A>`, `done: u16`, `needed: u16`, all public. Re-exported at the root: `Work`, `WorkKind`, `WorkKindId`.

- [ ] **Step 1: Write the module with its failing tests**

Create `crates/rl-rules/src/work.rs`:

```rust
//! Work: one thing an actor does across many turns.
//!
//! [`Work`] is how far along it is, counted in the worker's own turns and
//! never in clock time, so a worker twice as fast finishes in half the
//! clock without this module knowing speed exists. [`Work::advance`]
//! counts a turn and answers with [`Progress`]. [`in_reach`] is the one
//! test of whether work can still be done: a worker within [`REACH`] of
//! its target, asked of where both are now rather than how they got
//! there. What a kind of work is called is a [`WorkKindId`], interned
//! from the word a panel shows, so there is no list of kinds to add to.

use rl_core::{Id, Point, geometry};

/// What a kind of work is called, as an interned name. Never constructed;
/// it only types [`WorkKindId`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WorkKind {}

/// A kind of work, interned from the word a panel shows for it.
pub type WorkKindId = Id<WorkKind>;

/// How far from its target work can be done, in cells, Chebyshev: beside
/// it or on it. Nothing asks for work from further off.
pub const REACH: i32 = 1;

/// Whether a worker at `worker` can work on a target at `target`.
pub fn in_reach(worker: Point, target: Point) -> bool {
    geometry::chebyshev(worker, target) <= REACH
}

/// Where a piece of work stands after a turn of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    /// Not done; this many of the worker's turns are left.
    Left(u16),
    /// That was the last turn it needed.
    Finished,
}

/// One thing an actor is doing across many turns, and how far along it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Work<A> {
    /// What kind of work, which is also what a panel calls it.
    pub kind: WorkKindId,
    /// What it is done to, if anything: work on a target breaks when the
    /// target is gone or out of [`REACH`].
    pub target: Option<A>,
    /// Turns worked so far.
    pub done: u16,
    /// Turns it takes, never fewer than one.
    pub needed: u16,
}

impl<A> Work<A> {
    /// Work of `kind` taking `needed` of the worker's turns, on nothing.
    /// Zero is taken as one, since work that takes no turns is not work.
    pub fn new(kind: WorkKindId, needed: u16) -> Self {
        Self { kind, target: None, done: 0, needed: needed.max(1) }
    }

    /// The same work, done to `target`.
    pub fn on(mut self, target: A) -> Self {
        self.target = Some(target);
        self
    }

    /// Turns still to work.
    pub fn left(&self) -> u16 {
        self.needed - self.done
    }

    /// Counts one turn of work.
    pub fn advance(&mut self) -> Progress {
        self.done = (self.done + 1).min(self.needed);
        if self.done == self.needed { Progress::Finished } else { Progress::Left(self.left()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_finishes_on_exactly_the_turn_it_needs_and_not_before() {
        for needed in 1..=40u16 {
            let mut work: Work<()> = Work::new(WorkKindId::from_raw(0), needed);
            for turn in 1..needed {
                assert_eq!(work.advance(), Progress::Left(needed - turn), "turn {turn} of {needed}");
            }
            assert_eq!(work.advance(), Progress::Finished, "turn {needed} of {needed}");
            assert_eq!(work.left(), 0);
        }
    }

    #[test]
    fn work_that_asks_for_no_turns_takes_one() {
        let mut work: Work<()> = Work::new(WorkKindId::from_raw(0), 0);
        assert_eq!(work.needed, 1);
        assert_eq!(work.advance(), Progress::Finished);
    }

    #[test]
    fn a_target_is_in_reach_on_it_or_beside_it_and_not_a_cell_further() {
        let c = Point::new(5, 5);
        for dx in -1..=1 {
            for dy in -1..=1 {
                assert!(in_reach(c, c.offset(dx, dy)), "({dx}, {dy})");
            }
        }
        assert!(!in_reach(c, c.offset(2, 0)));
        assert!(!in_reach(c, c.offset(2, 2)));
    }
}
```

- [ ] **Step 2: Register the module**

In `crates/rl-rules/src/lib.rs`, add `pub mod work;` after `pub mod status;`, `pub use work::{Work, WorkKind, WorkKindId};` after the `status` re-export, and a line naming `work` in the crate's `//!` module list, matching how its neighbours are named.

- [ ] **Step 3: Run the tests and the wasm build**

Run: `cargo test -p rl-rules work` then `scripts/check-tiers.sh --wasm`
Expected: PASS, and `rl-rules` still builds for wasm.

- [ ] **Step 4: Commit** (no changelog line: nothing a game uses yet)

```bash
git add crates/rl-rules/src/work.rs crates/rl-rules/src/lib.rs
git commit -m "rl-rules: work counted in the worker's own turns, and the one cell it reaches"
```

---

## Task 5: Work is begun and carried on

**Files:**
- Create: `crates/rl-bevy/src/work.rs`
- Modify: `crates/rl-rules/src/ai/brain.rs` (`Decision`), `crates/rl-bevy/src/minds.rs` (`MindIntents`, `decide_minds`, `MindsPlugin::build`), `crates/rl-bevy/src/lib.rs`
- Test: `crates/rl-bevy/src/work.rs`, `crates/rl-rules/src/ai/brain.rs`
- Docs: `CHANGELOG.md`

**Interfaces:**
- Consumes: `rl_rules::work::*` (Task 4).
- Produces: `Decision::Work(Work<A>)`; in `rl_bevy::work`: `WorkPlugin`; `WorkKinds` with `declare(&mut self, &str) -> WorkKindId`, `get(&self, &str) -> Option<WorkKindId>`, `name(&self, WorkKindId) -> &str`; `trait AddWork { fn add_work(&mut self, name: &str) -> &mut Self; }`; `Working(pub Work<Entity>)`; actions `BeginWork(pub Work<Entity>)` and `Toil`; messages `WorkBegan { actor, kind, target }` and `WorkDone { actor, kind, target }`; `Works` system param with `begin(&mut self, actor: Entity, work: Work<Entity>)`; systems `continue_work`, `resolve_begins`, `resolve_toil`.

- [ ] **Step 1: The decision**

In `crates/rl-rules/src/ai/brain.rs`, import `crate::work::Work` and add a variant to `Decision` before `Own`:

```rust
    /// Begin work of many turns. The engine spends this turn as its first
    /// and every turn of the actor's after it on the rest, without asking
    /// the brain again, until it is done or broken.
    Work(Work<A>),
```

and an arm to its `PartialEq`:

```rust
            (Decision::Work(a), Decision::Work(b)) => a == b,
```

Add to that file's tests:

```rust
    #[test]
    fn two_decisions_to_work_are_equal_only_when_the_work_is() {
        let kind = crate::work::WorkKindId::from_raw(0);
        let a: Decision<u32> = Decision::Work(Work::new(kind, 4).on(7));
        assert_eq!(a, Decision::Work(Work::new(kind, 4).on(7)));
        assert_ne!(a, Decision::Work(Work::new(kind, 5).on(7)));
        assert_ne!(a, Decision::Work(Work::new(kind, 4).on(8)));
    }
```

- [ ] **Step 2: Write the failing tests**

Create `crates/rl-bevy/src/work.rs` with only its tests module for now (the implementation follows in Step 4):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::Health;
    use crate::components::{Actor, Blocks, Player, Position, Speed, Viewshed};
    use crate::minds::{Mind, MindsPlugin, Perception, Thinking};
    use crate::plugin::{PerceiveSet, Turn, TurnSet, headless_app};
    use crate::state::EngineState;
    use crate::turn::{Intent, Turns, Wait};
    use rl_core::Point;
    use rl_rules::ai::{Brain, Decision, Tactic, TacticCtx};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A world with work in it and every plugin a test here needs, all
    /// added before the app first runs; a player to hold the turns; the
    /// word the work is called by; and the sides and damage kind.
    fn arena() -> (App, Point, Entity, WorkKindId) {
        let mut app = headless_app();
        app.add_plugins((
            crate::fov::FovPlugin,
            crate::world::StreamingPlugin,
            crate::combat::CombatPlugin,
            MindsPlugin,
            crate::remains::RemainsPlugin,
            WorkPlugin,
        ));
        app.add_work("mending");
        let kind = app.world().resource::<WorkKinds>().get("mending").expect("declared");
        let start = crate::testing::surface(&mut app);
        let sides = crate::testing::two_sides(&mut app);
        app.insert_resource(TheSides(sides));
        let player = app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(8), Health::full(30), crate::combat::Faction(sides.ours))).id();
        app.init_resource::<Finished>().add_systems(Turn, record_finished.in_set(TurnSet::Cleanup));
        app.init_resource::<Opened>().add_systems(Turn, count_openings.in_set(PerceiveSet::Annotate));
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        (app, start, player, kind)
    }

    /// The sides `arena` made, for a test that needs a faction or a damage kind.
    #[derive(Resource, Clone, Copy)]
    struct TheSides(crate::testing::Sides);

    /// Every `WorkDone`, with the clock it was written at.
    #[derive(Resource, Default)]
    struct Finished(Vec<(WorkDone, u32)>);

    fn record_finished(mut done: MessageReader<WorkDone>, turns: Res<Turns>, mut seen: ResMut<Finished>) {
        for d in done.read() {
            seen.0.push((*d, turns.now()));
        }
    }

    /// One player turn: a wait, if the player holds the turn, and a frame.
    fn pass(app: &mut App, player: Entity) {
        if app.world().get::<crate::components::MyTurn>(player).is_some() {
            app.world_mut().write_message(Intent::new(player, Wait));
        }
        app.update();
    }

    #[test]
    fn work_ends_after_exactly_the_turns_it_needs_and_the_clock_it_took_scales_with_speed() {
        for speed in [50, 100, 200] {
            for needed in 1..=6u16 {
                let (mut app, start, player, kind) = arena();
                let worker = app.world_mut().spawn((Actor, Blocks, Position(start.offset(3, 0)), Speed(speed), Health::full(5))).id();
                app.update();
                let began = app.world().resource::<Turns>().now();
                app.world_mut().entity_mut(worker).insert(Working(Work::new(kind, needed)));
                for _ in 0..(needed as usize * 4 + 8) {
                    pass(&mut app, player);
                }
                let finished = &app.world().resource::<Finished>().0;
                assert_eq!(finished.len(), 1, "one finish, speed {speed}, needed {needed}");
                let each = rl_core::turn::scaled_cost(rl_core::turn::BASE_ACTION_COST, speed);
                assert!(finished[0].1 - began <= needed as u32 * each, "{needed} turns at speed {speed} take no more than {needed} of its turns");
                assert!(finished[0].1 - began + each >= needed as u32 * each, "and no fewer");
                assert!(app.world().get::<Working>(worker).is_none(), "and nothing is left working");
            }
        }
    }

    /// Asks once and counts each asking, so a test sees whether the brain
    /// was consulted.
    struct Mend {
        kind: WorkKindId,
        needed: u16,
        asked: Arc<AtomicU32>,
    }

    impl Tactic<Entity> for Mend {
        fn name(&self) -> &'static str {
            "mend"
        }
        fn evaluate(&self, _: &mut TacticCtx<'_, Entity>) -> Option<Decision<Entity>> {
            self.asked.fetch_add(1, Ordering::Relaxed);
            Some(Decision::Work(Work::new(self.kind, self.needed)))
        }
    }

    #[derive(Resource, Default)]
    struct Opened(u32);

    /// Counts every time the perceive stage opens a snapshot.
    fn count_openings(mut thinking: ResMut<Thinking>, mut opened: ResMut<Opened>) {
        if thinking.snapshot_mut().is_some() {
            opened.0 += 1;
        }
    }

    #[test]
    fn a_mind_that_chose_work_is_not_asked_again_and_perceives_nothing_until_it_is_done() {
        let (mut app, start, player, kind) = arena();
        let asked = Arc::new(AtomicU32::new(0));
        let brain = Arc::new(Brain::new().then(Mend { kind, needed: 5, asked: asked.clone() }));
        app.world_mut().spawn((Actor, Blocks, Position(start.offset(3, 0)), Health::full(5), Perception(6), Mind(brain)));
        for _ in 0..20 {
            if !app.world().resource::<Finished>().0.is_empty() {
                break;
            }
            pass(&mut app, player);
        }
        assert_eq!(app.world().resource::<Finished>().0.len(), 1, "the work was finished");
        assert_eq!(asked.load(Ordering::Relaxed), 1, "and the brain was asked once, on the turn it chose the work");
        assert_eq!(app.world().resource::<Opened>().0, 1, "nor did it perceive anything while it worked");
        for _ in 0..3 {
            pass(&mut app, player);
        }
        assert!(asked.load(Ordering::Relaxed) >= 2, "done, it is asked again");
    }

    #[test]
    fn work_of_one_turn_is_done_on_the_turn_it_is_begun_and_leaves_nothing_working() {
        let (mut app, start, player, kind) = arena();
        let asked = Arc::new(AtomicU32::new(0));
        let brain = Arc::new(Brain::new().then(Mend { kind, needed: 1, asked }));
        let mender = app.world_mut().spawn((Actor, Blocks, Position(start.offset(3, 0)), Health::full(5), Perception(6), Mind(brain))).id();
        pass(&mut app, player);
        assert!(!app.world().resource::<Finished>().0.is_empty(), "done at once");
        assert!(app.world().get::<Working>(mender).is_none(), "with no work left hanging");
    }
}
```

- [ ] **Step 3: Run them to see them fail**

Run: `cargo test -p rl-bevy work::`
Expected: FAIL to compile.

- [ ] **Step 4: Write the plugin**

Above the tests in `crates/rl-bevy/src/work.rs`:

```rust
//! Work: an actor doing one thing across many turns.
//!
//! A mind begins work by deciding [`Decision::Work`](rl_rules::ai::Decision::Work),
//! which becomes a [`BeginWork`] intent; anything else begins it through
//! [`Works::begin`]. Either way the turn it is begun on is its first, and
//! the actor carries [`Working`] until it is done. On every turn after,
//! [`continue_work`] claims the actor's decision before any mind is asked
//! and writes a [`Toil`], so a busy actor never thinks, and
//! [`resolve_toil`] spends the turn as a wait costs and counts it. On the
//! last it writes [`WorkDone`], which is where a game says what finishing
//! means. [`WorkBegan`] is written when work starts. What a kind of work
//! is called is interned by [`WorkKinds`] from the word a panel shows,
//! through [`AddWork::add_work`]. [`WorkPlugin`] is opt-in.

use bevy::prelude::*;
use rl_core::Interner;
use rl_core::turn::BASE_ACTION_COST;
use rl_rules::work::{Progress, Work, WorkKind, WorkKindId};

use crate::components::{MyTurn, Player};
use crate::plugin::{DecideSet, ResolveSet, Turn};
use crate::turn::{Acting, Action, AddAction, Intent, Resolution};

/// Every kind of work in play, by the word a panel shows for it.
///
/// Interned rather than free strings so a game compares a [`WorkDone`]'s
/// kind by id, and a save stores the word, so the order kinds were
/// declared in never reaches a save.
#[derive(Resource, Debug, Clone, Default)]
pub struct WorkKinds(Interner<WorkKind>);

impl WorkKinds {
    /// The id for `name`, assigning a new one if it is unseen.
    pub fn declare(&mut self, name: &str) -> WorkKindId {
        self.0.intern(name)
    }

    /// The id for `name`, if it has been declared.
    pub fn get(&self, name: &str) -> Option<WorkKindId> {
        self.0.get(name)
    }

    /// The word behind `id`.
    pub fn name(&self, id: WorkKindId) -> &str {
        self.0.name(id)
    }
}

/// Declares a kind of work while the app is being built.
pub trait AddWork {
    /// Declares the kind of work called `name`, the word a panel shows
    /// while an actor does it. Look its id up with [`WorkKinds::get`].
    fn add_work(&mut self, name: &str) -> &mut Self;
}

impl AddWork for App {
    fn add_work(&mut self, name: &str) -> &mut Self {
        self.init_resource::<WorkKinds>();
        self.world_mut().resource_mut::<WorkKinds>().declare(name);
        self
    }
}

/// The work an actor is in the middle of.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Working(pub Work<Entity>);

/// Begin work: what a mind's [`Decision::Work`](rl_rules::ai::Decision::Work) becomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BeginWork(pub Work<Entity>);
impl Action for BeginWork {}

/// Spend a turn on the work in hand. Written by [`continue_work`] and
/// never by a game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Toil;
impl Action for Toil {}

/// Work was begun, this pass.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkBegan {
    /// Who.
    pub actor: Entity,
    /// What kind.
    pub kind: WorkKindId,
    /// On what, if anything.
    pub target: Option<Entity>,
}

/// Work was finished, this pass. What finishing means is the game's.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkDone {
    /// Who.
    pub actor: Entity,
    /// What kind.
    pub kind: WorkKindId,
    /// On what, if anything.
    pub target: Option<Entity>,
}

/// Begins work, for anything that is not a mind's decision.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Works<'w, 's> {
    commands: Commands<'w, 's>,
    began: MessageWriter<'w, WorkBegan>,
    done: MessageWriter<'w, WorkDone>,
}

impl Works<'_, '_> {
    /// Begins `work` for `actor`, counting this turn as its first, so the
    /// caller is spending a turn on it. Work of one turn is done at once
    /// and leaves nothing working.
    pub fn begin(&mut self, actor: Entity, mut work: Work<Entity>) {
        self.began.write(WorkBegan { actor, kind: work.kind, target: work.target });
        match work.advance() {
            Progress::Finished => {
                self.done.write(WorkDone { actor, kind: work.kind, target: work.target });
            }
            Progress::Left(_) => {
                self.commands.entity(actor).insert(Working(work));
            }
        }
    }
}

/// Carries on the work of whoever holds the turn, before any mind is
/// asked: claiming the decision is what keeps the perceive stage shut.
///
/// Not the player's: the player's own long actions wait for a slice of
/// their own, with a key to stop them.
pub fn continue_work(mut acting: ResMut<Acting>, mut toil: MessageWriter<Intent<Toil>>, working: Query<Entity, (With<Working>, With<MyTurn>, Without<Player>)>) {
    for actor in &working {
        if acting.claim_decision(actor) {
            toil.write(Intent::new(actor, Toil));
        }
    }
}

/// Begins the work a mind decided on, spending the turn as a wait does.
pub fn resolve_begins(mut intents: MessageReader<Intent<BeginWork>>, mut resolution: Resolution, mut works: Works) {
    for intent in intents.read() {
        if resolution.claim(intent.actor) {
            works.begin(intent.actor, intent.action.0);
            resolution.done(intent.actor, BASE_ACTION_COST);
        }
    }
}

/// Spends a turn on the work in hand, as a wait costs, and finishes it on
/// its last.
pub fn resolve_toil(
    mut commands: Commands,
    mut intents: MessageReader<Intent<Toil>>,
    mut resolution: Resolution,
    mut working: Query<&mut Working>,
    mut done: MessageWriter<WorkDone>,
) {
    for intent in intents.read() {
        let Ok(mut work) = working.get_mut(intent.actor) else { continue };
        if !resolution.claim(intent.actor) {
            continue;
        }
        if work.0.advance() == Progress::Finished {
            commands.entity(intent.actor).remove::<Working>();
            done.write(WorkDone { actor: intent.actor, kind: work.0.kind, target: work.0.target });
        }
        resolution.done(intent.actor, BASE_ACTION_COST);
    }
}

/// Work: an actor doing one thing across many turns.
///
/// Opt-in. A mind that decides to work in a game without it is refused
/// by the sweeper, the way a blow is in a game without combat.
pub struct WorkPlugin;

impl Plugin for WorkPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorkKinds>()
            .add_message::<WorkBegan>()
            .add_message::<WorkDone>()
            .add_action::<BeginWork>()
            .add_action::<Toil>()
            .add_systems(Turn, continue_work.in_set(DecideSet::Sense))
            .add_systems(Turn, (resolve_begins, resolve_toil).chain().in_set(ResolveSet::Act));
    }
}
```

In `crates/rl-bevy/src/lib.rs`, add `pub mod work;` in order, and re-export `work::{AddWork, BeginWork, Toil, WorkBegan, WorkDone, WorkKinds, WorkPlugin, Working, Works}` at the root and `AddWork, WorkPlugin, Working, WorkKinds` in the prelude, beside `remains`.

- [ ] **Step 5: A mind's decision becomes the intent**

In `crates/rl-bevy/src/minds.rs`, add to `MindIntents`:

```rust
    begins: MessageWriter<'w, Intent<crate::work::BeginWork>>,
```

an arm to the match in `decide_minds` before `Decision::Own`:

```rust
        Decision::Work(work) => {
            intents.begins.write(Intent::new(thinker, crate::work::BeginWork(work)));
        }
```

and, in `MindsPlugin::build` beside `.add_action::<Attack>()`:

```rust
            // Work a mind decides in a game without `WorkPlugin` is refused,
            // not left to hang, for the same reason as a blow.
            .add_action::<crate::work::BeginWork>()
```

- [ ] **Step 6: Run the tests**

Run: `cargo test -p rl-rules brain` and `cargo test -p rl-bevy`
Expected: PASS.

- [ ] **Step 7: Changelog and commit**

```markdown
- Work: an actor doing one thing across many turns. `WorkPlugin`, opt-in, with `app.add_work("word")` declaring a kind by the word a panel shows. A mind begins work by deciding `Decision::Work(Work::new(kind, turns).on(target))`, and anything else through `Works::begin`; the turn it is begun on is its first, and every turn after is spent on it without asking the brain, until `WorkDone`. `rl_rules::work` is the model. `Decision::Work` is new, so a `match` on `Decision` gains an arm.
```

```bash
git add crates/rl-rules/src/ai/brain.rs crates/rl-bevy/src/work.rs crates/rl-bevy/src/minds.rs crates/rl-bevy/src/lib.rs CHANGELOG.md
git commit -m "work: an actor can spend many turns on one thing without being asked again"
```

---

## Task 6: Work breaks off

**Files:**
- Modify: `crates/rl-bevy/src/work.rs`
- Test: `crates/rl-bevy/src/work.rs`
- Docs: `CHANGELOG.md`

**Interfaces:**
- Consumes: Task 5's types; `DamageDealt`, `DeathEvent` from `crate::combat`; `Reads` from `crate::plugin`; `in_reach` from `rl_rules::work`.
- Produces: `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum BreakReason { Hurt, Died, OutOfReach, DoneByAnother, Stopped }`; `#[derive(Message)] pub struct WorkBroken { pub actor: Entity, pub kind: WorkKindId, pub target: Option<Entity>, pub done: u16, pub reason: BreakReason }`; `Works::stop(&mut self, actor: Entity)`; the system `break_work`.

- [ ] **Step 1: Write the failing tests**

In `arena`, after the `Finished` line, add:

```rust
        app.init_resource::<Broken>().add_systems(Turn, record_broken.in_set(TurnSet::Cleanup));
```

and to the tests module:

```rust
    use crate::combat::DamageEvent;
    use rl_rules::Hit;

    /// Every `WorkBroken`, in order.
    #[derive(Resource, Default)]
    struct Broken(Vec<WorkBroken>);

    fn record_broken(mut broken: MessageReader<WorkBroken>, mut seen: ResMut<Broken>) {
        seen.0.extend(broken.read().copied());
    }

    /// A world with combat, a worker three cells from the player working
    /// on a thing beside it, and the kind of damage to hurt it with.
    fn at_work() -> (App, Entity, Entity, Entity, rl_rules::DamageKindId) {
        let (mut app, start, player, kind) = arena();
        let sides = app.world().resource::<TheSides>().0;
        let thing = app.world_mut().spawn(Position(start.offset(4, 0))).id();
        let worker = app.world_mut().spawn((Actor, Blocks, Position(start.offset(3, 0)), Health::full(10), crate::combat::Faction(sides.theirs))).id();
        app.world_mut().entity_mut(worker).insert(Working(Work::new(kind, 50).on(thing)));
        app.update();
        (app, player, worker, thing, sides.kind)
    }

    fn reasons(app: &App) -> Vec<BreakReason> {
        app.world().resource::<Broken>().0.iter().map(|b| b.reason).collect()
    }

    #[test]
    fn a_worker_that_is_hurt_breaks_off_even_when_healed_in_the_same_pass() {
        let (mut app, player, worker, _, damage) = at_work();
        app.world_mut().write_message(DamageEvent::new(worker, Hit::from_source(None, damage, 2)));
        app.world_mut().write_message(DamageEvent::new(worker, Hit::from_source(None, damage, -2)));
        pass(&mut app, player);
        assert_eq!(reasons(&app), vec![BreakReason::Hurt]);
        assert!(app.world().get::<Working>(worker).is_none());
    }

    #[test]
    fn a_worker_killed_at_work_breaks_off_as_dead_not_hurt() {
        let (mut app, player, worker, _, damage) = at_work();
        app.world_mut().write_message(DamageEvent::new(worker, Hit::from_source(None, damage, 99)));
        pass(&mut app, player);
        assert_eq!(reasons(&app), vec![BreakReason::Died]);
    }

    #[test]
    fn a_worker_moved_out_of_reach_breaks_off_and_one_moved_within_reach_does_not() {
        let (mut app, player, worker, thing, _) = at_work();
        let there = app.world().get::<Position>(thing).unwrap().0;
        // Swapped to the other side of it, still beside it.
        app.world_mut().get_mut::<Position>(worker).unwrap().0 = there.offset(1, 1);
        pass(&mut app, player);
        assert!(reasons(&app).is_empty(), "still within reach");
        app.world_mut().get_mut::<Position>(worker).unwrap().0 = there.offset(3, 0);
        pass(&mut app, player);
        assert_eq!(reasons(&app), vec![BreakReason::OutOfReach], "shoved off");
    }

    #[test]
    fn work_on_a_target_carried_off_or_gone_breaks_off() {
        let (mut app, player, _, thing, _) = at_work();
        app.world_mut().get_mut::<Position>(thing).unwrap().0.x += 3;
        pass(&mut app, player);
        assert_eq!(reasons(&app), vec![BreakReason::OutOfReach], "carried off");

        let (mut app, player, _, thing, _) = at_work();
        app.world_mut().despawn(thing);
        pass(&mut app, player);
        assert_eq!(reasons(&app), vec![BreakReason::OutOfReach], "gone");
    }

    #[test]
    fn when_one_worker_finishes_a_target_every_other_on_it_breaks_off() {
        let (mut app, player, slow, thing, _) = at_work();
        let kind = app.world().resource::<WorkKinds>().get("mending").unwrap();
        let at = app.world().get::<Position>(thing).unwrap().0;
        let quick = app.world_mut().spawn((Actor, Blocks, Position(at.offset(0, 1)), Health::full(10))).id();
        app.world_mut().entity_mut(quick).insert(Working(Work::new(kind, 2).on(thing)));
        for _ in 0..4 {
            pass(&mut app, player);
        }
        let broken = &app.world().resource::<Broken>().0;
        assert_eq!(broken.len(), 1);
        assert_eq!((broken[0].actor, broken[0].reason), (slow, BreakReason::DoneByAnother));
    }

    #[derive(Resource)]
    struct StopNow(Entity);

    fn stop_it(stop: Option<Res<StopNow>>, mut works: Works) {
        if let Some(stop) = stop {
            works.stop(stop.0);
        }
    }

    #[test]
    fn a_game_can_stop_work_and_says_so() {
        let (mut app, player, worker, _, _) = at_work();
        app.add_systems(Turn, stop_it.in_set(TurnSet::React));
        app.insert_resource(StopNow(worker));
        pass(&mut app, player);
        assert_eq!(reasons(&app), vec![BreakReason::Stopped]);
        assert!(app.world().get::<Working>(worker).is_none());
    }

    /// The twin is taken before the work breaks, so an actor stood back up
    /// comes back doing what it was doing, and breaks off at once if what
    /// it was working on has gone.
    #[test]
    fn an_actor_revived_comes_back_at_the_work_it_died_doing() {
        let (mut app, player, worker, thing, damage) = at_work();
        app.world_mut().entity_mut(worker).insert(crate::remains::LeavesRemains);
        app.world_mut().write_message(DamageEvent::new(worker, Hit::from_source(None, damage, 99)));
        pass(&mut app, player);
        assert!(crate::remains::revive(app.world_mut(), worker, 5));
        assert!(app.world().get::<Working>(worker).is_some(), "back at its work");

        app.world_mut().despawn(thing);
        pass(&mut app, player);
        assert_eq!(reasons(&app).last(), Some(&BreakReason::OutOfReach), "which it drops once the thing is gone");
    }
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p rl-bevy work::`
Expected: FAIL to compile, `WorkBroken` not found.

- [ ] **Step 3: The reasons, the message, and `stop`**

Add to `crates/rl-bevy/src/work.rs`:

```rust
/// Why work was broken off.
///
/// Closed, because each is a thing the engine itself detects; a rule of
/// a game's own stops work with [`Works::stop`] and reads as `Stopped`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakReason {
    /// The worker was harmed: any damage that landed, even a hit healed in
    /// the same pass, since the hit is what breaks concentration.
    Hurt,
    /// The worker died. A killing blow is harm too, and reads as this.
    Died,
    /// The target is gone, or it and the worker are further apart than
    /// [`REACH`](rl_rules::work::REACH).
    OutOfReach,
    /// Another worker finished work on the same target.
    DoneByAnother,
    /// The game stopped it.
    Stopped,
}

/// Work was broken off, this pass, and everything done on it is lost.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkBroken {
    /// Who.
    pub actor: Entity,
    /// What kind.
    pub kind: WorkKindId,
    /// On what, if anything.
    pub target: Option<Entity>,
    /// Turns it had done, for a game that wants a half-done job remembered.
    pub done: u16,
    /// Why.
    pub reason: BreakReason,
}
```

Extend `Works` with `broken: MessageWriter<'w, WorkBroken>` and `working: Query<'w, 's, &'static Working>`, and add:

```rust
    /// Stops `actor`'s work, if it has any, as a rule of the game's own.
    pub fn stop(&mut self, actor: Entity) {
        let Ok(work) = self.working.get(actor) else { return };
        self.commands.entity(actor).remove::<Working>();
        self.broken.write(WorkBroken { actor, kind: work.0.kind, target: work.0.target, done: work.0.done, reason: BreakReason::Stopped });
    }
```

- [ ] **Step 4: The one system that breaks work**

```rust
/// Breaks off work for every reason the engine detects, once per worker,
/// the gravest reason first.
///
/// In `TurnSet::React`, which the turn loop runs after the whole resolve
/// chain: after damage has landed, and after `RemainsPlugin` has kept a
/// twin of anyone who died, so a worker stood back up comes back at its
/// work. Harm and death are read from the one place each lands, not
/// watched for; reach is asked of where things are now, so every way of
/// moving a worker or its target is covered by one question, and it is
/// asked every pass so a panel never shows work that can no longer be done.
pub fn break_work(
    mut commands: Commands,
    mut hurt: MessageReader<DamageDealt>,
    mut deaths: MessageReader<DeathEvent>,
    mut finished: MessageReader<WorkDone>,
    mut broken: MessageWriter<WorkBroken>,
    working: Query<(Entity, &Working, &Position, Option<&OnMap>)>,
    places: Query<(&Position, Option<&OnMap>)>,
) {
    fn note(reasons: &mut Vec<(Entity, BreakReason)>, actor: Entity, reason: BreakReason) {
        if !reasons.iter().any(|(a, _)| *a == actor) {
            reasons.push((actor, reason));
        }
    }
    let map_of = |on: Option<&OnMap>| on.map(|m| m.0).unwrap_or(MapId::SURFACE);
    let mut reasons: Vec<(Entity, BreakReason)> = Vec::new();
    for death in deaths.read() {
        note(&mut reasons, death.entity, BreakReason::Died);
    }
    for harm in hurt.read().filter(|h| h.dealt > 0) {
        note(&mut reasons, harm.target, BreakReason::Hurt);
    }
    let done: Vec<(Entity, Option<Entity>)> = finished.read().map(|d| (d.actor, d.target)).collect();
    for (actor, work, pos, on) in &working {
        let Some(target) = work.0.target else { continue };
        if done.iter().any(|(finisher, t)| *finisher != actor && *t == Some(target)) {
            note(&mut reasons, actor, BreakReason::DoneByAnother);
            continue;
        }
        let reachable = places.get(target).is_ok_and(|(there, target_on)| map_of(target_on) == map_of(on) && in_reach(pos.0, there.0));
        if !reachable {
            note(&mut reasons, actor, BreakReason::OutOfReach);
        }
    }
    for (actor, reason) in reasons {
        let Ok((_, work, _, _)) = working.get(actor) else { continue };
        commands.entity(actor).remove::<Working>();
        broken.write(WorkBroken { actor, kind: work.0.kind, target: work.0.target, done: work.0.done, reason });
    }
}
```

with imports `crate::combat::{DamageDealt, DeathEvent}`, `crate::components::{OnMap, Position}`, `crate::plugin::{Reads, TurnSet}`, `rl_core::MapId`, `rl_rules::work::in_reach`.
In `WorkPlugin::build` add `.add_message::<WorkBroken>()`, `.reads::<DamageDealt>().reads::<DeathEvent>()` (so a game with no combat has empty queues rather than a panic), and `.add_systems(Turn, break_work.in_set(TurnSet::React))`.
Name `BreakReason`, `WorkBroken`, `Works::stop` and `break_work` in the module `//!` docs, and add `BreakReason, WorkBroken` to the `lib.rs` re-exports.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p rl-bevy`
Expected: PASS.

- [ ] **Step 6: Changelog and commit**

```markdown
- Work breaks off, with `WorkBroken { reason }`, when the worker is hurt, even by a hit healed in the same pass; when it dies; when its target is gone or more than one cell away, however either got there; when another worker finishes the same target; or when the game calls `Works::stop`. Progress is lost with it, and `WorkBroken::done` says how far it got.
```

```bash
git add crates/rl-bevy/src/work.rs crates/rl-bevy/src/lib.rs CHANGELOG.md
git commit -m "work breaks off when the worker is hurt or killed, out of reach, beaten to it, or stopped"
```

---

## Task 7: Work is saved

**Files:**
- Modify: `crates/rl-save/src/run.rs` (`EntityState`, `of`, `restore`, the test `game`)
- Test: `crates/rl-save/src/run.rs` tests
- Docs: `CHANGELOG.md`

**Interfaces:**
- Consumes: `Working`, `WorkKinds`, `WorkPlugin`, `AddWork` (Task 5).
- Produces: `EntityState::working: Option<SavedWork>` and `pub struct SavedWork { pub kind: String, pub target: Option<SaveId>, pub done: u16, pub needed: u16 }`.

- [ ] **Step 1: Write the failing test**

In the tests module, add `WorkPlugin` to the plugins `game()` adds and `.add_work("mending")` after them. Then:

```rust
    /// Work saved is work continued, on the same target and as far along;
    /// work on a target that was not saved is dropped, since there is
    /// nothing left to work on.
    #[test]
    fn work_saved_is_work_continued_and_work_on_something_unsaved_is_dropped() {
        let backend = std::sync::Arc::new(MemoryBackend::default());
        let (mut app, start) = game(Saves(backend.clone()));
        app.world_mut().spawn((Actor, Player, Blocks, You, Position(start), Viewshed::new(6), RevealsMap, Health::full(30)));
        let kind = app.world().resource::<rl_bevy::WorkKinds>().get("mending").unwrap();
        let bo = app.world_mut().spawn((Actor, Blocks, Person("Bo".into()), Position(start.offset(1, 3)), Health::full(20))).id();
        let loose = app.world_mut().spawn(Position(start.offset(4, 4))).id();
        let mut work = rl_rules::Work::new(kind, 9).on(bo);
        work.done = 4;
        let ada = app.world_mut().spawn((Actor, Blocks, Person("Ada".into()), Position(start.offset(0, 3)), Health::full(20), rl_bevy::Working(work))).id();
        app.world_mut().spawn((Actor, Blocks, Person("Cy".into()), Position(start.offset(4, 3)), Health::full(20), rl_bevy::Working(rl_rules::Work::new(kind, 9).on(loose))));
        play(&mut app);
        // However far play carried it, what comes back is what was saved.
        let saved = app.world().get::<rl_bevy::Working>(ada).expect("Ada is at work when saved").0;
        save_run(app.world_mut()).unwrap();

        let (mut back, _) = game(Saves(backend));
        load_run(back.world()).unwrap().expect("a save").restore(back.world_mut()).unwrap();
        let w = back.world_mut();
        let people: Vec<(Entity, String)> = w.query::<(Entity, &Person)>().iter(w).map(|(e, p)| (e, p.0.clone())).collect();
        let who = |name: &str| people.iter().find(|(_, n)| n == name).unwrap().0;
        let w = back.world();
        let ada = w.get::<rl_bevy::Working>(who("Ada")).expect("Ada is still at work").0;
        assert_eq!((ada.kind, ada.target, ada.done, ada.needed), (kind, Some(who("Bo")), saved.done, 9), "on Bo, as far in as when saved");
        assert!(w.get::<rl_bevy::Working>(who("Cy")).is_none(), "Cy's work was on something that was not saved");
    }
```

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test -p rl-save work_saved_is_work_continued`
Expected: FAIL, Ada has no `Working` after the load.

- [ ] **Step 3: Save and restore the work**

Add beside `EntityState`:

```rust
/// Work an actor was in the middle of, as a save keeps it: the kind by its
/// word, so the order kinds were declared in never matters, and the target
/// by save id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedWork {
    /// The word the kind was declared by.
    pub kind: String,
    /// What it was done to, if anything.
    pub target: Option<SaveId>,
    /// Turns done.
    pub done: u16,
    /// Turns it takes.
    pub needed: u16,
}
```

a field at the end of `EntityState`:

```rust
    /// What it was in the middle of, for an actor at work.
    #[serde(default)]
    pub working: Option<SavedWork>,
```

in `of`:

```rust
        let working = match (e.get::<Working>(), world.get_resource::<WorkKinds>()) {
            (Some(w), Some(kinds)) => Some(SavedWork {
                kind: kinds.name(w.0.kind).to_string(),
                target: w.0.target.map(|t| remap.save_id(t)),
                done: w.0.done,
                needed: w.0.needed,
            }),
            _ => None,
        };
```

with `working,` in the struct literal, and in `restore`, before the remains block:

```rust
        // Work on something that did not come back is dropped: there is
        // nothing left to work on. A kind the game no longer declares is
        // dropped the same way, since nothing would ever finish it.
        if let Some(saved) = &self.working
            && let Some(kind) = world.get_resource::<WorkKinds>().and_then(|k| k.get(&saved.kind))
        {
            let target = saved.target.map(|id| remap.entity(id));
            if target != Some(None)
                && let Ok(mut worker) = world.get_entity_mut(entity)
            {
                worker.insert(Working(Work { kind, target: target.flatten(), done: saved.done, needed: saved.needed }));
            }
        }
```

importing `Working`, `WorkKinds` from `rl_bevy` and `Work` from `rl_rules`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p rl-save`
Expected: PASS, including every existing test, since the field defaults.

- [ ] **Step 5: Changelog and commit**

```markdown
- `rl-save` keeps an actor's work, the kind by its word and the target by save id, and drops it on load when the target was not saved. A save written before this loads with no one at work.
```

```bash
git add crates/rl-save/src/run.rs CHANGELOG.md
git commit -m "rl-save: work saved is work continued, and work on something unsaved is dropped"
```

---

## Task 8: What the player sees

**Files:**
- Modify: `crates/rl-ui/src/view/mod.rs`, `crates/rl-ui/src/view/nearby.rs`, `crates/rl-ui/src/panel/nearby.rs`, `crates/rl-ui/src/view/inspect.rs`, `crates/rl-ui/src/panel/inspect.rs`, `crates/rl-ui/src/lib.rs` (exports)
- Test: each of those files' tests modules
- Docs: `CHANGELOG.md`

**Interfaces:**
- Consumes: `Working`, `WorkKinds` (Task 5).
- Produces: `pub struct WorkRow { pub doing: String, pub target: Option<String>, pub left: u16 }`; `Row::work: Option<WorkRow>`; `pub struct Workings` system param with `row(&self, Entity) -> Option<WorkRow>`; `InspectLayout::working: String` and `working_alone: String`; `InspectPanel::working(self, with_target, alone) -> Self`; `pub fn working_line(template: &str, work: &WorkRow) -> String`.

- [ ] **Step 1: Write the failing tests**

In `crates/rl-ui/src/view/nearby.rs` tests:

```rust
    #[test]
    fn a_row_says_what_an_actor_is_working_on_and_how_long_it_has_left() {
        let mut stage = Stage::new_with(NearbyViewPlugin, |app| {
            app.add_plugins(rl_bevy::WorkPlugin).add_work("mending");
        });
        let kind = stage.app.world().resource::<rl_bevy::WorkKinds>().get("mending").unwrap();
        let mender = stage.actor("mender", 'm', 2, 0);
        let rag = stage.actor("rag doll", 'r', 3, 0);
        let mut work = rl_rules::Work::new(kind, 10).on(rag);
        work.done = 6;
        stage.app.world_mut().entity_mut(mender).insert(rl_bevy::Working(work));
        stage.tick();
        let view = stage.app.world().resource::<NearbyView>();
        let row = view.actors.iter().find(|r| r.entity == mender).unwrap();
        assert_eq!(row.work, Some(WorkRow { doing: "mending".into(), target: Some("rag doll".into()), left: 4 }));
        assert_eq!(view.actors.iter().find(|r| r.entity == rag).unwrap().work, None, "and nothing on one not at work");
    }
```

In `crates/rl-ui/src/panel/nearby.rs` tests, beside the `(hunting)` test:

```rust
    #[test]
    fn what_an_actor_is_working_at_is_written_where_its_alert_would_be() {
        let mut stage = Stage::new(NearbyPanel::new(Rect::new(0, 0, 24, 10)).titled("")).screen(24, 10);
        stage.actor("drone", 'u', 2, 0);
        stage.tick();
        let mut row = stage.app.world().resource::<NearbyView>().actors[0].clone();
        row.alert = Some(Alert::Hunting);
        row.work = Some(WorkRow { doing: "mending".into(), target: None, left: 3 });
        let palette = stage.app.world().resource::<Palette>().clone();
        let mut terminal = Terminal::new(24, 1, bevy::math::Vec2::ONE);
        draw_row(&mut terminal, Rect::new(0, 0, 24, 1), 0, &row, false, &AlertWords::default(), &palette);
        let said: String = (0..24).filter_map(|x| terminal.get(x, 0).map(|c| c.glyph)).collect();
        assert!(said.contains("drone (mending)"), "{said:?}");
        assert!(!said.contains("hunting"), "what it is busy with, not what it knows: {said:?}");
    }
```

In `crates/rl-ui/src/panel/inspect.rs` tests:

```rust
    #[test]
    fn the_working_line_reads_as_a_sentence_and_counts_turns_right() {
        let work = |target: Option<&str>, left| WorkRow { doing: "mending".into(), target: target.map(String::from), left };
        let layout = InspectPanel::new(Rect::new(0, 0, 30, 12)).0;
        assert_eq!(working_line(&layout.working, &work(Some("rag doll"), 4)), "Mending the rag doll, 4 turns left");
        assert_eq!(working_line(&layout.working, &work(Some("rag doll"), 1)), "Mending the rag doll, 1 turn left");
        assert_eq!(working_line(&layout.working_alone, &work(None, 2)), "Mending, 2 turns left");
        assert_eq!(working_line("{doing}: {n} cycles", &work(None, 2)), "Mending: 2 cycles", "a game's own template, with the bare number");
    }
```

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p rl-ui`
Expected: FAIL to compile.

- [ ] **Step 3: The row field and the param that fills it**

In `crates/rl-ui/src/view/mod.rs`:

```rust
/// What an actor is working at, as a panel reads it.
///
/// A field of the view rather than a facet, because the engine knows it
/// and every game with work would push the same one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkRow {
    /// The word the game declared the kind of work by.
    pub doing: String,
    /// What the work is done to, by name, if it is done to something.
    pub target: Option<String>,
    /// Of the worker's own turns, how many are left.
    pub left: u16,
}

/// Reads what an entity is working at, for any collector.
///
/// Empty in a game without work: no `Working` is ever put on anything, and
/// without `WorkKinds` there is no word to show.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Workings<'w, 's> {
    working: Query<'w, 's, &'static rl_bevy::Working>,
    kinds: Option<Res<'w, rl_bevy::WorkKinds>>,
    names: Query<'w, 's, &'static Name>,
}

impl Workings<'_, '_> {
    /// What `entity` is working at, if anything.
    pub fn row(&self, entity: Entity) -> Option<WorkRow> {
        let work = self.working.get(entity).ok()?.0;
        let kinds = self.kinds.as_deref()?;
        let target = work.target.and_then(|t| self.names.get(t).ok()).map(|n| n.as_str().to_string());
        Some(WorkRow { doing: kinds.name(work.kind).to_string(), target, left: work.left() })
    }
}
```

Add to `Row`, after `alert`:

```rust
    /// What it is working at, when it is at work. Written where the alert
    /// would be: what it is busy doing is what a player needs to know.
    pub work: Option<WorkRow>,
```

with `work: None` in `Row::new`. Export `WorkRow` and `Workings` wherever `Row` is exported in `crates/rl-ui/src/lib.rs`.

- [ ] **Step 4: The nearby collector and presenter**

Add `work: crate::view::Workings<'w, 's>,` to `Around`, and in `collect_nearby` inside `if sighting.actor {`, after the alert: `row.work = around.work.row(sighting.entity);`.

In `draw_row` in `crates/rl-ui/src/panel/nearby.rs`, replace the alert block with:

```rust
    // What it is doing, in the panel's own words and after the name: a
    // state a player reads rather than a mark they learn. What it is busy
    // with outranks what it knows of you, since something at work is not
    // coming for anyone, whatever it has noticed.
    let state = row.work.as_ref().map(|w| w.doing.as_str()).or_else(|| row.alert.and_then(|alert| words.get(alert)));
    if let Some(word) = state {
        name.push_str(" (");
        name.push_str(word);
        name.push(')');
    }
```

- [ ] **Step 5: The inspect line**

Add `work: crate::view::Workings<'w, 's>,` to `Duelists` (its sixteenth field; if the derive refuses, move `fire` and `gases` into a nested `#[derive(SystemParam)]` of their own), and in `collect_inspect` after the relation is set: `row.work = duelists.work.row(entity);`.

In `crates/rl-ui/src/panel/inspect.rs`, add to `InspectLayout`:

```rust
    /// What someone at work on something is said to be doing: `{doing}`
    /// is the kind's word, `{target}` what it is working on, `{left}` the
    /// turns left counted ("4 turns", "1 turn") and `{n}` the bare number.
    /// The first letter is capitalised.
    pub working: String,
    /// The same, for work done to nothing in particular.
    pub working_alone: String,
```

defaulting in `InspectPanel::new` to `"{doing} the {target}, {left} left"` and `"{doing}, {left} left"`, and a builder:

```rust
    /// Sets how work is described: with a target, and without one.
    pub fn working(mut self, with_target: impl Into<String>, alone: impl Into<String>) -> Self {
        self.0.working = with_target.into();
        self.0.working_alone = alone.into();
        self
    }
```

and the function:

```rust
/// One line saying what someone is working at, from a template.
pub fn working_line(template: &str, work: &WorkRow) -> String {
    let left = if work.left == 1 { "1 turn".to_string() } else { format!("{} {}", work.left, rl_core::noun::plural("turn")) };
    let line = template
        .replace("{doing}", &work.doing)
        .replace("{target}", work.target.as_deref().unwrap_or(""))
        .replace("{left}", &left)
        .replace("{n}", &work.left.to_string());
    let mut chars = line.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => line,
    }
}
```

In `draw_inspect`, right after the whereabouts line:

```rust
    if let Some(work) = &subject.work
        && y < inner.bottom()
    {
        let template = if work.target.is_some() { &layout.working } else { &layout.working_alone };
        terminal.print_on(inner.x, y, &clip(&working_line(template, work), width), palette.get(Tones::NOTICE), bg);
        y += 1;
    }
```

- [ ] **Step 6: Run the tests**

Run: `cargo test -p rl-ui`
Expected: PASS.

- [ ] **Step 7: Changelog and commit**

```markdown
- A row says what an actor is working at: `Row::work`, a `WorkRow` of the kind's word, the target's name and the turns left, filled by the nearby and inspect collectors through `Workings`. The nearby rail writes the work's word where it would write the alert word, so a drone at work reads `(mending)` rather than `(hunting)`. Inspect adds a line, "Mending the rag doll, 4 turns left", from templates a game replaces with `InspectPanel::working(with_target, alone)`. A `Row` built by hand gains the field.
```

```bash
git add crates/rl-ui CHANGELOG.md
git commit -m "a row says what an actor is working at, and inspect says how long it has left"
```

---

## Task 9: Foundry's repair drone

**Files:**
- Create: `examples/foundry/src/droids/repair.rs`
- Modify: `examples/foundry/src/droids.rs` (`mod repair;` and its re-exports, `MonsterDef::repairs`, the brain), `examples/foundry/src/plugin.rs` (plugins and systems), `examples/foundry/assets/monsters.ron`, `examples/foundry/assets/monster_spawns.ron`
- Test: `examples/foundry/src/droids/repair.rs`
- Docs: `CHANGELOG.md`

**Interfaces:**
- Consumes: `WorkPlugin`, `AddWork`, `WorkKinds`, `WorkDone`, `Working`, `ReviveCommands`, `Remains`, `Thinking`, `Sight`, `PerceiveSet::Annotate`; Foundry's `Roster`, `Kind`, `Faction`.
- Produces: `pub const REPAIRING: &str = "repairing"`; `Wrecks`; systems `sense_wrecks`, `rebuild_wrecks`; tactic `RepairWrecks`; `MonsterDef::repairs: Option<u16>`.

- [ ] **Step 1: Write the failing tests**

Create `examples/foundry/src/droids/repair.rs` with its tests module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{clear_droids, droid_down_a_lane, headless, hit, settle};
    use bevy::ecs::world::CommandQueue;
    use rl_engine::rl_bevy::prelude::*;

    /// A line droid wrecked five cells east of the commando, and a repair
    /// drone two cells past it, on the cleared lane.
    fn a_wreck_and_a_drone() -> (App, Entity, Entity, Entity) {
        let mut app = headless(RunSeed(2));
        let (droid, player) = droid_down_a_lane(&mut app, "line droid", 5, 8);
        clear_droids(&mut app, &[droid]);
        hit(&mut app, droid, "kinetic", 1_000);
        settle(&mut app);
        assert!(app.world().get::<Remains>(droid).is_some(), "a wreck");
        let registries = app.world().resource::<Registries>().clone();
        let roster = Roster::load(&registries);
        let at = app.world().get::<Position>(player).unwrap().0.offset(7, 0);
        let map = app.world().resource::<WorldMap>().current();
        let mut queue = CommandQueue::default();
        let mut commands = Commands::new(&mut queue, app.world_mut());
        let drone = crate::droids::spawn_monster(&mut commands, &roster, roster.defs.expect("repair drone"), at, map, &registries);
        queue.apply(app.world_mut());
        (app, player, droid, drone)
    }

    fn wait(app: &mut App, player: Entity) {
        if app.world().get::<MyTurn>(player).is_some() {
            app.world_mut().write_message(Intent::new(player, Wait));
        }
        app.update();
    }

    #[test]
    fn a_repair_drone_walks_to_a_wreck_of_its_own_side_and_stands_it_back_up() {
        let (mut app, player, droid, drone) = a_wreck_and_a_drone();
        let mut seen_working = false;
        for _ in 0..40 {
            wait(&mut app, player);
            seen_working |= app.world().get::<Working>(drone).is_some();
            if app.world().get::<Actor>(droid).is_some() {
                break;
            }
        }
        assert!(seen_working, "the drone set to work on it");
        let world = app.world();
        assert!(world.get::<Remains>(droid).is_none(), "the wreck is a droid again");
        assert_eq!(world.get::<Health>(droid).map(|h| (h.current, h.max)), Some((4, 8)), "at half its health, rounded up");
        assert_eq!(world.get::<Name>(droid).map(|n| n.as_str().to_string()), Some("line droid".into()));
    }

    #[test]
    fn a_repair_drone_shot_at_work_breaks_off_and_runs() {
        let (mut app, player, droid, drone) = a_wreck_and_a_drone();
        for _ in 0..20 {
            wait(&mut app, player);
            if app.world().get::<Working>(drone).is_some() {
                break;
            }
        }
        assert!(app.world().get::<Working>(drone).is_some(), "at work");
        let before = app.world().get::<Position>(drone).unwrap().0;
        let commando = app.world().get::<Position>(player).unwrap().0;
        hit(&mut app, drone, "kinetic", 3);
        wait(&mut app, player);
        assert!(app.world().get::<Working>(drone).is_none(), "shot, it stops");
        for _ in 0..3 {
            wait(&mut app, player);
        }
        let after = app.world().get::<Position>(drone).unwrap().0;
        assert!(
            rl_core::geometry::chebyshev(after, commando) > rl_core::geometry::chebyshev(before, commando),
            "and runs from the commando rather than going back to work at half health"
        );
        assert!(app.world().get::<Remains>(droid).is_some(), "the wreck stays a wreck");
    }
}
```

Adjust imports to what `droid_down_a_lane` and its neighbours in `examples/foundry/src/testing/droids.rs` use (`RunSeed`, `Registries`, `WorldMap`, `MyTurn`, `Intent`, `Wait`, `rl_core`), the same way that file imports them.

- [ ] **Step 2: Run them to see them fail**

Run: `cargo test -p foundry repair`
Expected: FAIL: no monster called "repair drone".

- [ ] **Step 3: The content**

In `examples/foundry/assets/monsters.ron`, add to the schema comment after `drops`:

```
//   repairs:    optional; the turns it takes to rebuild a wreck of its own side, plus one for every
//               two points of the wrecked kind's hp; a monster with it walks to such wrecks it can
//               see and rebuilds them rather than hunting, and the droid stands up at half its hp
```

and the row, after "heavy droid":

```ron
    // Rebuilds what the commando wrecks, and runs when shot at.
    (name: "repair drone", glyph: 'u', color: (0.55, 0.8, 0.75), hp: 6, armor: 0, profile: "chassis", faction: "droids", wits: ["mindless", "opens_doors"],
     perception: 8, dark_sight: 4, notice: (certain: 2, chance_pct: 35, lit_bonus: 6), speed: 100, flee_at: 50, hearing: (threshold: 0, memory: 8), drops: [("slug", 10)], repairs: 4),
```

In `examples/foundry/assets/monster_spawns.ron`, after the heavy droid rows:

```ron
    // Not before the wrecks are worth rebuilding.
    (monster: "repair drone",  decks: (3, 10), weight: 25,  group: (1, 1)),
```

In `MonsterDef`, after `drops`:

```rust
    /// The turns it takes to rebuild a wreck of its own side, before the
    /// wreck's own share; present, it rebuilds wrecks rather than hunting.
    #[serde(default)]
    pub repairs: Option<u16>,
```

- [ ] **Step 4: The sense, the tactic, and the answer to `WorkDone`**

Above the tests in `examples/foundry/src/droids/repair.rs`:

```rust
//! The repair drone: what it knows about wrecks, how it picks one, and
//! what rebuilding one means.
//!
//! The engine owns the work: the turns, the breaking off, the row that
//! says `(repairing)`. What Foundry owns is which wrecks are worth
//! rebuilding, how long each takes, and that a finished one stands up as
//! the droid it was.

use bevy::prelude::*;
use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_bevy::{ReviveCommands, Sight, Thinking, WorkDone, WorkKinds};
use rl_engine::rl_core::Point;
use rl_engine::rl_rules::ai::{Decision, Tactic, TacticCtx};
use rl_engine::rl_rules::work::{Work, WorkKindId, in_reach};

use super::{Kind, Roster};

/// The kind of work a repair drone does, by the word its row shows.
pub const REPAIRING: &str = "repairing";

/// The wrecks of its own side a repair drone can see, with the turns each
/// would take it.
#[derive(Debug, Clone)]
pub struct Wrecks {
    /// The kind of work, so the tactic needs no resource.
    pub kind: WorkKindId,
    /// Each wreck, where it lies, and how long rebuilding it takes.
    pub found: Vec<(Entity, Point, u16)>,
}

// ANCHOR: sense
/// Tells a repair drone holding the turn which wrecks of its own side it
/// can see, and how long each would take: its own `repairs`, plus one
/// turn for every two points of the wrecked kind's health.
pub fn sense_wrecks(
    mut thinking: ResMut<Thinking>,
    sight: Sight,
    roster: Res<Roster>,
    kinds: Res<WorkKinds>,
    me: Query<(&Kind, &Faction)>,
    wrecks: Query<(Entity, &Position, Option<&OnMap>, &Kind, &Faction), With<Remains>>,
) {
    let Some(actor) = thinking.actor() else { return };
    let Ok((my_kind, my_side)) = me.get(actor) else { return };
    let (Some(base), Some(kind)) = (roster.defs.get(my_kind.0).repairs, kinds.get(REPAIRING)) else { return };
    let found = wrecks
        .iter()
        .filter(|(.., side)| side.0 == my_side.0)
        .filter(|(_, pos, on, ..)| sight.perceives(&thinking, pos.0, *on))
        .map(|(wreck, pos, _, kind, _)| (wreck, pos.0, base + (roster.defs.get(kind.0).hp.max(0) as u16) / 2))
        .collect();
    if let Some(snapshot) = thinking.snapshot_mut() {
        snapshot.add_sense(Wrecks { kind, found });
    }
}
// ANCHOR_END: sense

// ANCHOR: tactic
/// Rebuild a wreck: set to work on it if it is within reach, and walk
/// toward the nearest otherwise.
pub struct RepairWrecks;

impl Tactic<Entity> for RepairWrecks {
    fn name(&self) -> &'static str {
        "repair_wrecks"
    }

    fn evaluate(&self, ctx: &mut TacticCtx<'_, Entity>) -> Option<Decision<Entity>> {
        let me = ctx.snapshot.me.pos;
        let wrecks = ctx.snapshot.sense::<Wrecks>()?.clone();
        if let Some((wreck, _, turns)) = wrecks.found.iter().find(|(_, at, _)| in_reach(me, *at)) {
            return Some(Decision::Work(Work::new(wrecks.kind, *turns).on(*wreck)));
        }
        let cells: Vec<Point> = wrecks.found.iter().map(|(_, at, _)| *at).collect();
        ctx.step_toward(&cells).map(Decision::Step)
    }
}
// ANCHOR_END: tactic

// ANCHOR: rebuild
/// A finished repair stands the wreck up as the droid it was, at half its
/// health, and says so when the commando can see it happen.
pub fn rebuild_wrecks(
    mut commands: Commands,
    mut done: MessageReader<WorkDone>,
    kinds: Res<WorkKinds>,
    roster: Res<Roster>,
    wrecks: Query<(&Kind, &Position), With<Remains>>,
    eyes: Query<&Viewshed, With<Player>>,
    mut tell: MessageWriter<Tell>,
) {
    let Some(repairing) = kinds.get(REPAIRING) else { return };
    for finished in done.read().filter(|d| d.kind == repairing) {
        let Some(wreck) = finished.target else { continue };
        let Ok((kind, pos)) = wrecks.get(wreck) else { continue };
        let def = roster.defs.get(kind.0);
        commands.revive(wreck, (def.hp + 1) / 2);
        if eyes.iter().any(|v| v.can_see(pos.0)) {
            tell.write(Tell::new(format!("The {} whirs back to life.", def.name), Tones::NOTICE));
        }
    }
}
// ANCHOR_END: rebuild
```

Use the import paths the heist uses for `Thinking`, `Sight` and `add_sense`, and the ones Foundry's `props.rs` uses for `Tell` and `Tones`; `Faction` and `Viewshed` come from the `rl_bevy` prelude or wherever `spawn_monster` takes them. If `Tell::new` takes `&str`, pass `&format!(..)`.

In `examples/foundry/src/droids.rs`, add `mod repair;` beside `mod alarm;`, `pub use repair::{REPAIRING, RepairWrecks, Wrecks, rebuild_wrecks, sense_wrecks};`, and in the brain building replace the `brain = match d.shadow { .. };` with:

```rust
            brain = match (d.repairs, d.shadow) {
                // A drone that rebuilds has nothing to hunt with.
                (Some(_), _) => brain.then(RepairWrecks),
                (None, Some(s)) => brain.then(Keep::enemies(s.keep_within, s.no_closer_than)).then(Hover),
                (None, None) => brain.then(Hunt),
            };
```

In `examples/foundry/src/plugin.rs`, beside `RemainsPlugin::naming(..)`:

```rust
        // Work, for the repair drone: the engine keeps its turns and its
        // row; `droids::repair` says which wrecks and what finishing means.
        app.add_plugins(WorkPlugin).add_work(crate::droids::REPAIRING);
        app.add_systems(Turn, crate::droids::sense_wrecks.in_set(PerceiveSet::Annotate))
            .add_systems(Turn, crate::droids::rebuild_wrecks.in_set(TurnSet::React));
```

If Foundry's ambiguity test (`examples/foundry/src/plugin/ambiguity.rs`) names a new pair, order the two with `.before`/`.after` on Foundry's own system and, only where both are engine systems, fix the order in the engine; never add the pair to an allow-list without a comment saying why no order is right.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p foundry`
Expected: the two new tests PASS. Foundry's seeded fingerprint tripwire fails, because a new spawn row changes what every deck from three draws; that is expected. Re-baseline it with the new value it prints, and put the old and the new value in the changelog line below, in the form the earlier re-baselines in `CHANGELOG.md` use ("re-baselined from 4522241706254112780 to 8405133597109566668").

- [ ] **Step 6: Changelog and commit**

```markdown
- Foundry has a repair drone, from deck three: it walks to a droid wreck it can see, rebuilds it over its `repairs` turns plus one for every two points of the wreck's health, and the droid stands back up at half its health; the rail reads `repair drone (repairing)`, and shooting it stops the work and sends it running. `monsters.ron` gains `repairs`. Its fingerprint tripwire is re-baselined, for the new spawn row.
```

```bash
git add examples/foundry CHANGELOG.md
git commit -m "foundry: a repair drone rebuilds the droids the commando wrecks, and runs when shot at"
```

---

## Task 10: In the running game, and the branch's documentation

**Files:** whatever the play-through finds wrong; the five documentation files `AGENTS.md` names.

This is the user's rule: the change is not done until it has been seen working the way a player would see it, and anything that looks off on screen gets fixed along the way.

- [ ] **Step 1: Launch Foundry**

Use the `run` skill to launch `cargo run -p foundry` and drive it. Start a new run and take the cheat lift (`\`) to deck three or deeper.

- [ ] **Step 2: A repair, seen**

Find or spawn (cheat menu) a repair drone and a line droid. Wreck the droid within the drone's sight and stand back. Check, with a screenshot of each:
- The drone walks to the wreck; the nearby rail reads `repair drone (repairing)` once it sets to work, in the drone's side colour, not muted.
- Open inspect (`l` or the game's look key) on the drone: "Repairing the line droid remains, N turns left", counting down one per turn, "1 turn left" on the last.
- The wreck stands up as `line droid` with its own glyph, half health on its row, and the log says "The line droid whirs back to life."

- [ ] **Step 3: Interruptions, seen**

Shoot a drone at work: the rail drops `(repairing)` in the same turn, and the drone runs. Loot a wreck (take everything, including anything it wore) and let a drone rebuild it: the droid stands up carrying nothing that is now in your pack. Stand on a wreck while a drone finishes it: the droid stands up beside you.

- [ ] **Step 4: A continued run**

With a drone mid-repair, quit to the menu and continue: the drone is still at work with the same turns left, and the wreck is still rebuilt when it finishes.

- [ ] **Step 5: Fix what looked off**

Anything misaligned, clipped, mis-toned or worded wrong on any screen seen above, related or not, is fixed now, with a test where one can be written, and committed on its own with a message saying what is now true.

- [ ] **Step 6: The documentation pass**

- `docs/guide/src/systems/work.md`, new: the six fixed parts, a `documents:` manifest naming `WorkPlugin` and every file it makes a claim about (`crates/rl-bevy/src/work.rs`, `crates/rl-rules/src/work.rs`, `crates/rl-rules/src/ai/brain.rs`, `crates/rl-ui/src/view/mod.rs`, `crates/rl-ui/src/panel/inspect.rs`, `crates/rl-save/src/run.rs`), and `Using it` quoting the `sense`, `tactic` and `rebuild` anchors from `examples/foundry/src/droids/repair.rs`. Copy the manifest and include shape from `docs/guide/src/systems/remains.md`. Add it to `docs/guide/src/SUMMARY.md`.
- `docs/guide/src/systems/remains.md`: revival in `The model`, the twin and `revive` in `The line`.
- Every other page `python3 scripts/check-systems.py` names: read the diff it prints against the page, fix what it made untrue, then bless it.
- `docs/OVERVIEW.md`: `WorkPlugin` in the `rl-bevy` plugin table; revival under remains; `Row::work` and the inspect line under `rl-ui`; the repair drone under Foundry.
- `README.md`'s feature list: one line for work across many turns and bodies that can be stood back up.
- `docs/design/work.md` and `docs/design/remains.md` §9: status lines to "built" and the date the branch finishes, and any place the built thing differs from the design, said.
- `docs/PLAN.md`'s progress log: one entry for the branch.
- `scripts/check-systems-style.sh docs/guide/src/systems/work.md` and `.../remains.md`.

Commit the documentation on its own: `docs: work and revival, in the guide, the overview and the plan`.

- [ ] **Step 7: Final checks**

Run every check in Global Constraints once more on the branch head, then hand off with superpowers:finishing-a-development-branch.
