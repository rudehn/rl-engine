# Worn gear, the unseen, and enchant by band: implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Foundry gains a nanite plate that mends its wearer on a clock and a cloak plate that, used while worn, makes its wearer unseen, both scaling with an enchant level rolled for the band they are found at, on engine mechanics every game can use.

**Architecture:** The engine gains four mechanics, each inside the subsystem that already owns its neighbours: a `pulse` moment and the use-while-worn rule in items, attunement in consumables, a level on every effect landing in effects, and the `unseen` status property in stealth. Loot hands the band to the game's `ItemMaker` through `Provenance`, and `rl-rules` gains a `LevelTable` a game rolls levels from. Foundry is the first game to use all of it.

**Tech Stack:** Rust 2024, Bevy ECS, `rand`, `serde`/`ron`, mdBook guide pages.

**Spec:** `docs/superpowers/specs/2026-09-26-worn-gear-and-the-unseen-design.md`. Read it before starting; this plan argues from it, and its section 9 lists what is deliberately not built.

## Global Constraints

- Never an em dash anywhere, code or prose; use a plain dash.
- Commit messages never carry a `Co-Authored-By` line naming an agent (the user's rule overrides the harness's reminder).
- Commit messages follow the repo's style: a lower-case sentence saying what is now true, as in `git log --oneline -10`.
- Work on a branch named `worn-gear` in a worktree (superpowers:using-git-worktrees). The main checkout has the user's uncommitted edits to `CHANGELOG.md`, `crates/rl-ui/src/cursor.rs`, `crates/rl-ui/src/view/inspect.rs`, `docs/guide/src/systems/controls.md`, `docs/guide/src/systems/panels.md` and `examples/foundry/TODO.md`; never touch that checkout.
- Every task's commit passes: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `scripts/check-tiers.sh`, `scripts/check-tiers.sh --wasm`, `scripts/check-overview.sh`, `python3 scripts/check-systems.py`, `scripts/check-guide.sh`.
- `rl-core`, `rl-grid` and `rl-rules` are tier 0 and 1: no Bevy, they build on `wasm32-unknown-unknown`, no `std::time::Instant`.
- `#![deny(missing_docs)]`: every public item gets a doc comment that says why, at the density of `crates/rl-core/src/turn.rs`, not more.
- No `HashMap`/`HashSet` on gameplay paths; `Vec` or `BTreeMap`. No `TODO` comments in source.
- Randomness only from the stream a system is handed (`RunSeed::derive`-derived); a level is rolled from the `rng` `ItemMaker::make` receives. A band whose level row has one entry draws nothing.
- Clocks and costs are integers in hundredths of a step: `Pulse::every`, `Recharge::every`.
- No theme words in engine crates: the engine says `unseen`, `pulse`, `attuned`; `cloak` and `nanite` are Foundry's.
- A system page (`docs/guide/src/systems/*.md`) whose manifest lists a file you changed must be re-read against the code, corrected, and blessed with `python3 scripts/check-systems.py --bless <system>`; then `scripts/check-systems-style.sh docs/guide/src/systems/<system>.md` must pass (80 lines of prose, six fixed parts, no invented code).
- Every RON schema carries a top-of-file comment listing the full option space; a new field is added to it in the same commit.
- `CHANGELOG.md` under `Unreleased` whenever a game on the previous release would change a line; each task below says whether it owes one.
- Long Markdown: one sentence per physical line.
- The numbers: nanite plate `pulse: (every: 1000, per_level: -100, fastest: 100)`, armor 1, `enchant: (most: 9)`, mends 1 `care`, found on decks 3 to 10; cloak plate armor 1, `attuned: true`, `consumable: (charges: 1, when_empty: Kept, recharge: 4000)`, `enchant: (most: 9)`, `Inflict` `cloaked` for 10 turns with `per_level: 2`, found on decks 1 to 10; `cloaked` badge `'%'`; the level table in spec section 7, `+1` about one find in ten on deck one and `+5` only at band 10.

## Review Focus

These are the inputs most likely to bite a player that no happy-path test exercises; each has its test in the owning task.

1. **A pulse on a wearer who is not on the current map**: it lands on nobody, never on whoever stands at the same coordinates here (Task 3).
2. **Put on, take off, put on again**: an attuned thing is empty after each putting-on however long it charged while worn, and gains nothing while off (Task 4).
3. **Throwing and aiming an ability at a foe end the unseen like a blow; a self-aimed ability does not** (Task 8).
4. **A band past the level table's last row**: rolls from the last row, not plain (Task 7).
5. **A continued run**: a thing's level, a pulse's progress and a charge's progress come back as they were (Tasks 3 and 9).

---

## File map

| File | Change | Responsibility |
|---|---|---|
| `crates/rl-bevy/src/combat.rs` | modify | a heal reports what it restored |
| `crates/rl-bevy/src/items.rs` | modify | use only while worn; `Pulse`, `pulse_worn`, `restart_pulses` |
| `crates/rl-bevy/src/effects/triggers.rs` | modify | `Moments::PULSE`; `land_triggers` reads the carrier's level |
| `crates/rl-bevy/src/effects/mod.rs` | modify | `Landing::level`; `Effect::describe` and `Effects::describe` take a level |
| `crates/rl-bevy/src/effects/engine.rs` | modify | `per_level` on `Harm`, `Mend`, `Inflict` |
| `crates/rl-bevy/src/ability.rs` | modify | an ability lands at level zero |
| `crates/rl-bevy/src/consumable.rs` | modify | `Attuned`, `attune`, recharge only while worn |
| `crates/rl-bevy/src/loot.rs` | modify | `Provenance`; every maker call passes its band |
| `crates/rl-bevy/src/prefabs.rs` | modify | a slot's items made at the slot's band |
| `crates/rl-bevy/src/stealth.rs` | modify | `Unseen`, `mark_unseen`, `reveal_attackers`, `filter_unseen`, awareness and watchers |
| `crates/rl-bevy/src/lib.rs` | modify | exports |
| `crates/rl-rules/src/status.rs` | modify | `StatusDef::unseen` |
| `crates/rl-rules/src/loot.rs` | modify | `LevelRow`, `LevelTable`, `load_levels` |
| `crates/rl-rules/src/lib.rs` | modify | exports |
| `crates/rl-save/src/engine.rs` | modify | a pulse's progress is saved |
| `crates/rl-ui/src/view/inventory.rs` | modify | `usable`, the pulse's lead-in, `ready_in`, `attuned`, lines at the item's level |
| `crates/rl-ui/src/panel/inventory.rs` | modify | the charging lines |
| `crates/rl-bevy/tests/genres.rs`, `examples/corsair/src/abilities.rs`, `examples/delve/src/effects.rs` | modify | `describe` takes a level |
| `examples/corsair/src/items.rs` | modify | `make` takes a `Provenance` |
| `examples/foundry/src/content.rs` | modify | the `cloaked` status |
| `examples/foundry/src/gear.rs` | modify | `pulse`, `attuned`, `enchant`; `levels.ron`; the maker rolls levels; `spawn_item_at` |
| `examples/foundry/src/save.rs` | modify | an item's level |
| `examples/foundry/src/run.rs`, `examples/foundry/src/plugin.rs` | modify | the commando drawn faded while unseen |
| `examples/foundry/assets/items.ron`, `item_spawns.ron`, `levels.ron` | modify / create | the plates, where they lie, the level table |
| docs listed per task | modify | the documentation each change owes |

---

## Task 1: A heal reports what it restored

**Files:**
- Modify: `crates/rl-bevy/src/combat.rs:407-420` (`DamageDealt::dealt` doc), `:873-899` (`apply_damage`)
- Test: `crates/rl-bevy/src/combat.rs` tests module
- Docs: `docs/guide/src/systems/combat.md` (bless), `CHANGELOG.md`

**Interfaces:**
- Produces: `DamageDealt::dealt` is, for a heal, the negative of the health actually gained. Task 3's pulse relies on a mend at full health being silent.

- [ ] **Step 1: Write the failing test**

Add to the tests module in `crates/rl-bevy/src/combat.rs`, beside the `duel` tests:

```rust
    /// A heal says what it restored rather than what it offered: two to
    /// bring twenty-eight back to thirty, and then nought, so a wearer who
    /// is whole is not told every few turns that they mended.
    #[test]
    fn a_heal_reports_what_it_restored_and_nothing_at_full_health() {
        let (mut app, _, player, _) = duel(4, false, |kind| MeleeAttack::new(kind, DiceRoll::flat(1)));
        let kind = app.world().resource::<Registries>().damage_kinds.expect("kinetic");
        app.world_mut().get_mut::<Health>(player).unwrap().current = 28;
        app.world_mut().write_message(DamageEvent::new(player, Hit::by(player, kind, -5)));
        app.update();
        app.world_mut().write_message(DamageEvent::new(player, Hit::by(player, kind, -5)));
        app.update();
        let dealt: Vec<i32> = app.world().resource::<Seen>().dealt.iter().map(|d| d.dealt).collect();
        assert_eq!(dealt, vec![-2, 0], "two to reach thirty, then nothing left to mend");
        assert_eq!(hp(&app, player), 30);
    }
```

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test -p rl-bevy a_heal_reports_what_it_restored`
Expected: FAIL, `left: [-5, -5]`.

- [ ] **Step 3: Report the health restored**

In `apply_damage`, replace the two lines that set health and write `DamageDealt`:

```rust
        let before = health.current;
        health.current = (health.current - amount).min(health.max);
        // A heal reports what it restored, not what it offered: a mend at
        // full health restored nothing, and saying otherwise put a line in
        // the log every time a worn thing mended a wearer who was whole.
        let reported = if amount < 0 { before - health.current } else { amount };
        dealt.write(DamageDealt { target: ev.target, hit: ev.hit, dealt: reported, reach: ev.reach });
```

And the field's doc on `DamageDealt`:

```rust
    /// What health lost; for a heal, the negative of what it gained, which
    /// is nought for a heal at full health.
    pub dealt: i32,
```

- [ ] **Step 4: Run the test and the workspace**

Run: `cargo test -p rl-bevy a_heal_reports_what_it_restored` then `cargo test --workspace`
Expected: PASS. If a test elsewhere asserted a heal's full offered amount on someone already whole (look at `crates/rl-bevy/src/effects/triggers.rs` near line 560 and `examples/corsair/src/items.rs` near line 614), its setup was at full health: wound the target first so the assertion still measures the heal, and say so in its comment.

- [ ] **Step 5: Docs**

Re-read `docs/guide/src/systems/combat.md`; if it describes `DamageDealt`, add: "A heal reports the health it restored, so a mend at full health reports nought." Run `python3 scripts/check-systems.py`, bless every page it names after re-reading each against the change (`python3 scripts/check-systems.py --bless combat`, and the same for each other page it lists), then `scripts/check-systems-style.sh docs/guide/src/systems/combat.md`.

Add under `Unreleased` in `CHANGELOG.md`:

```markdown
- A heal's `DamageDealt::dealt` is the health it restored, so a mend at full health reports nought and the narrator says nothing; it was the amount offered. A game that read a heal's `dealt` as what was offered reads it off the `DamageEvent` instead.
```

- [ ] **Step 6: Commit**

```bash
git add crates/rl-bevy/src/combat.rs docs/guide/src/systems CHANGELOG.md
git commit -m "a heal reports the health it restored, and nothing at full health"
```

---

## Task 2: A worn thing is used only while it is worn

**Files:**
- Modify: `crates/rl-bevy/src/items.rs:1-21` (module doc), `:240-249` (`UseItem` doc), `:386-400` (`Which::Use`)
- Modify: `crates/rl-ui/src/view/inventory.rs:90-99` (`ItemRow::usable`)
- Test: `crates/rl-bevy/src/items.rs` tests, `crates/rl-ui/src/view/inventory.rs` tests
- Docs: `docs/guide/src/systems/items.md` (bless), `docs/design/items.md` section 4, `CHANGELOG.md`

**Interfaces:**
- Produces: `UseItem(item)` of a `Wearable` item not in the user's `Equipped` is refused (`Resolution::failed`, free for the player). `ItemRow::usable()` is false for a wearable row that is not worn. Task 9's cloak plate relies on both.

- [ ] **Step 1: Write the failing engine test**

Add to the tests module in `crates/rl-bevy/src/items.rs`:

```rust
    /// A thing that is worn is used by wearing it: in the bag its use is
    /// refused and costs nothing, and once it is on the use goes through.
    #[test]
    fn a_wearable_thing_is_used_only_while_it_is_worn() {
        let mut r = rig();
        let plate = r.app.world_mut().spawn((Item, Position(r.start), Wearable(EquipShape::in_slot(r.main)))).id();
        act(&mut r, PickUp);
        let before = r.app.world().resource::<Turns>().now();
        assert!(act(&mut r, UseItem(plate)).is_empty(), "in the bag, it is not used");
        assert_eq!(r.app.world().resource::<Turns>().now(), before, "and the refusal is free");
        act(&mut r, Equip(plate));
        assert_eq!(act(&mut r, UseItem(plate)), vec![ItemEvent::Used { actor: r.player, item: plate }], "worn, it is");
    }
```

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test -p rl-bevy a_wearable_thing_is_used_only_while_it_is_worn`
Expected: FAIL, the first `act` returned a `Used` event.

- [ ] **Step 3: Refuse a use off the body**

In `resolve_items`, replace the `Which::Use(item)` arm:

```rust
                Which::Use(item) => {
                    let Ok((pos, bag, worn)) = carriers.get_mut(actor) else { break 'attempt false };
                    // An empty wand is still a wand, and using one is a
                    // mistake the player keeps the turn for, as for any
                    // impossible item action.
                    let empty = consumables.get(item).is_ok_and(|c| c.is_empty());
                    // A thing that can be worn is used by wearing it: one
                    // used from the bottom of the bag would let a wearer
                    // keep one plate on and spend another's charge.
                    let off_the_body = wearables.contains(item) && !worn.is_some_and(|w| w.slot_of(item).is_some());
                    if bag.contains(item) && !empty && !off_the_body {
                        events.write(ItemEvent::Used { actor, item });
                        fired.write(crate::effects::Fired { on: item, moment: crate::effects::Moments::USE, by: Some(actor), at: pos.0 });
                        true
                    } else {
                        false
                    }
                }
```

Extend `UseItem`'s doc with: "A thing that can be worn is used only while it is worn, and a use of one in the bag is refused the same way."
Extend the module doc's first paragraph after "An item never lends an ability." with: "A thing that can be worn is used only while it is worn."

- [ ] **Step 4: Run the engine test**

Run: `cargo test -p rl-bevy a_wearable_thing_is_used_only_while_it_is_worn`
Expected: PASS.

- [ ] **Step 5: Write the failing view test**

Add to the tests module in `crates/rl-ui/src/view/inventory.rs`:

```rust
    /// The bag offers the use key for a worn thing only while it is on,
    /// the rule the items resolver refuses it by.
    #[test]
    fn a_wearable_thing_is_usable_only_while_it_is_worn() {
        let mut stage = Stage::new_with(InventoryViewPlugin, |app| {
            app.world_mut().resource_mut::<Registries>().slots = Registry::from_defs(vec![SlotDef::new("torso")]).unwrap();
        });
        let player = stage.player;
        let torso = stage.app.world().resource::<Registries>().slots.expect("torso");
        let on_use = rl_bevy::Trigger {
            on: rl_bevy::Moments::USE,
            area: rl_rules::Area::Here,
            fires: None,
            effects: std::sync::Arc::new(rl_bevy::Effects::default()),
            look: None,
        };
        let plate = stage.app.world_mut().spawn((Item, Name::new("plate"), Wearable(EquipShape::in_slot(torso)), rl_bevy::Triggers(vec![on_use]))).id();
        stage.app.world_mut().entity_mut(player).insert((Inventory { items: vec![plate] }, Equipped(Equipment::with_slot_count(1))));
        stage.tick();
        assert!(!stage.app.world().resource::<InventoryView>().rows[0].usable(), "in the bag, the use key is not offered");
        stage.app.world_mut().get_mut::<Equipped>(player).unwrap().equip(plate, &EquipShape::in_slot(torso)).unwrap();
        stage.tick();
        assert!(stage.app.world().resource::<InventoryView>().rows[0].usable(), "worn, it is");
    }
```

- [ ] **Step 6: Run it to see it fail, then make `usable` say so**

Run: `cargo test -p rl-ui a_wearable_thing_is_usable_only_while_it_is_worn`
Expected: FAIL on the first assertion.

Replace `ItemRow::usable` and its doc:

```rust
    /// Whether the use key does anything to it: a `use` trigger, a charge
    /// to spend when it counts them, and, for a thing that can be worn,
    /// being worn, which is when the items resolver lets it be used.
    ///
    /// A thing whose effects all keep quiet about themselves is still used:
    /// the row reads it off the component, not off the description, so a
    /// game that wrote a terse effect does not lose the key that uses it.
    pub fn usable(&self) -> bool {
        self.uses_something && !self.empty && (!self.wearable() || self.worn())
    }
```

Run: `cargo test -p rl-ui a_wearable_thing_is_usable_only_while_it_is_worn`
Expected: PASS.

- [ ] **Step 7: Docs**

In `docs/design/items.md` section 4, after "The user is the only target and their own cell the only cell.", add the line:

```markdown
A thing that can be worn is used only while it is worn: a plate carried in the bag and used would let a wearer keep one plate on and spend another's charge, which is the swap the attunement in section 5 exists to prevent.
```

Re-read `docs/guide/src/systems/items.md`; in `The model`, after the sentence "A use of an empty `Consumable` is impossible, so it costs the player nothing.", add: "So is a use of a thing that can be worn and is not, since a worn thing is used by wearing it." Run `python3 scripts/check-systems.py`, re-read and bless every page it names, and run `scripts/check-systems-style.sh` on each.

Add under `Unreleased` in `CHANGELOG.md`:

```markdown
- A thing that can be worn is used only while it is worn: `UseItem` of a `Wearable` not in the user's `Equipped` is refused for free, and `ItemRow::usable` is false for it. No game shipped a wearable thing with a `use` trigger, so nothing changes for one that did not.
```

- [ ] **Step 8: Commit**

```bash
git add crates/rl-bevy/src/items.rs crates/rl-ui/src/view/inventory.rs docs CHANGELOG.md
git commit -m "a thing that can be worn is used only while it is worn"
```

---

## Task 3: A worn thing's pulse

**Files:**
- Modify: `crates/rl-bevy/src/effects/triggers.rs:38-60` (`Moments`)
- Modify: `crates/rl-bevy/src/items.rs` (`Pulse`, `pulse_worn`, `restart_pulses`, `ItemsPlugin::build`)
- Modify: `crates/rl-bevy/src/lib.rs:73-76` and the prelude's `items` line (export `Pulse`)
- Modify: `crates/rl-save/src/engine.rs:100-150` (`EffectState`)
- Modify: `crates/rl-ui/src/view/inventory.rs` (`lead_in`, `Does`, `collect_inventory`)
- Test: `crates/rl-bevy/src/consumable.rs` tests (its `rig` already lands effects on a wounded player), `crates/rl-bevy/src/effects/triggers.rs` tests, `crates/rl-save/src/engine.rs` tests, `crates/rl-ui/src/view/inventory.rs` tests
- Docs: `docs/guide/src/systems/items.md`, `effects.md`, `saving.md` (bless), `docs/design/items.md` section 6, `docs/design/effects.md` section 2, `CHANGELOG.md`

**Interfaces:**
- Consumes: Task 1's silent zero heal.
- Produces:
  - `Moments::PULSE: MomentId` (raw 6), and `Moments::BUILT_IN` gains `"pulse"` at the end.
  - `pub struct Pulse { pub every: u32, pub progress: u32 }` with `Pulse::every(every: u32) -> Pulse`, in `rl_bevy::items`, re-exported as `rl_bevy::Pulse` and in the prelude.
  - `pub fn pulse_worn(...)`, `pub fn restart_pulses(...)`.
  - `EffectState::pulse: Option<u32>`.

- [ ] **Step 1: Write the failing moment test**

Add to the tests module in `crates/rl-bevy/src/effects/triggers.rs`:

```rust
    /// The pulse is the engine's seventh moment, added at the end so every
    /// moment before it keeps the id a save or a content file already has.
    #[test]
    fn the_pulse_is_a_built_in_moment_after_the_six_that_came_first() {
        let moments = Moments::default();
        assert_eq!(moments.get("pulse"), Some(Moments::PULSE));
        assert_eq!(Moments::PULSE, MomentId::from_raw(6));
        assert_eq!(moments.get("destroyed"), Some(Moments::DESTROYED), "the old ids did not move");
    }
```

Run: `cargo test -p rl-bevy the_pulse_is_a_built_in_moment`
Expected: FAIL to compile, `no associated item named PULSE`.

- [ ] **Step 2: Add the moment**

In `Moments`:

```rust
    /// A worn thing's own clock came round: see
    /// [`Pulse`](crate::items::Pulse).
    pub const PULSE: MomentId = MomentId::from_raw(6);

    /// The engine's moments, in the order their ids are handed out.
    pub const BUILT_IN: [&'static str; 7] = ["use", "land", "fire", "hit", "entered", "destroyed", "pulse"];
```

Update the `Moment` doc to "What sets a trigger off: a thing used, a throw come to rest, an attack made or landed, a cell stepped on, a prop broken, a worn thing's clock, or one a game names."

Run: `cargo test -p rl-bevy the_pulse_is_a_built_in_moment`
Expected: PASS.

- [ ] **Step 3: Write the failing pulse tests**

In the tests module of `crates/rl-bevy/src/consumable.rs`, add these helpers after `health`:

```rust
    /// Gives the player one slot and makes `item` wearable in it.
    fn a_slot_for(app: &mut App, player: Entity, item: Entity) {
        let slot = rl_rules::SlotId::from_raw(0);
        app.world_mut().entity_mut(item).insert(crate::items::Wearable(rl_rules::EquipShape::in_slot(slot)));
        app.world_mut().entity_mut(player).insert(crate::items::Equipped(rl_rules::Equipment::with_slot_count(1)));
    }

    /// Puts `item` on through the engine's own intent.
    fn put_on(app: &mut App, player: Entity, item: Entity) {
        app.world_mut().write_message(Intent::new(player, crate::items::Equip(item)));
        app.update();
    }

    /// Takes `item` off through the engine's own intent.
    fn take_off(app: &mut App, player: Entity, item: Entity) {
        app.world_mut().write_message(Intent::new(player, crate::items::Unequip(item)));
        app.update();
    }

    /// Waits until the clock reads at least `until`.
    fn wait_until(app: &mut App, player: Entity, until: u32) {
        while app.world().resource::<Turns>().now() < until {
            app.world_mut().write_message(Intent::new(player, Wait));
            app.update();
        }
    }

    /// A trigger list that mends one on each pulse.
    const MEND_ON_PULSE: &str = r#"[(on: "pulse", effects: [(kind: "Mend", args: (kind: "care", roll: "1"))])]"#;

    /// `text`'s triggers, built against the rig's moments and effect kinds,
    /// the way the rig builds its own item's.
    fn triggers_of(app: &App, text: &str) -> Triggers {
        let specs: Vec<rl_rules::TriggerSpec> =
            ron::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME).from_str(text).expect("the triggers parse");
        let world = app.world();
        Triggers::build(&specs, &[], world.resource::<Moments>(), world.resource::<EffectKinds>(), &world.resource::<Registries>().names()).expect("the triggers build")
    }
```

Then the tests:

```rust
    /// A worn thing's pulse mends its wearer once for every period it has
    /// been worn, counted from the moment it went on, and not at all while
    /// it sits in the bag.
    #[test]
    fn a_worn_pulse_lands_once_a_period_from_when_it_went_on_and_never_from_the_bag() {
        let (mut app, player, plate) = rig(None, None, MEND_ON_PULSE);
        app.world_mut().entity_mut(plate).insert(crate::items::Pulse::every(800));
        a_slot_for(&mut app, player, plate);
        wait_until(&mut app, player, 1000);
        assert_eq!(health(&app, player), 10, "ten turns in the bag and nothing");
        let on = app.world().resource::<Turns>().now();
        put_on(&mut app, player, plate);
        wait_until(&mut app, player, on + 700);
        assert_eq!(health(&app, player), 10, "seven turns worn, not yet");
        wait_until(&mut app, player, on + 800);
        assert_eq!(health(&app, player), 11, "eight turns worn, one mended");
        wait_until(&mut app, player, on + 1600);
        assert_eq!(health(&app, player), 12, "and one more eight turns on");
    }

    /// Taking a worn thing off and putting it back starts its clock again:
    /// seven turns worn, off, and on again is not one turn from a mend.
    #[test]
    fn putting_a_pulsing_thing_back_on_starts_its_clock_again() {
        let (mut app, player, plate) = rig(None, None, MEND_ON_PULSE);
        app.world_mut().entity_mut(plate).insert(crate::items::Pulse::every(800));
        a_slot_for(&mut app, player, plate);
        let on = app.world().resource::<Turns>().now();
        put_on(&mut app, player, plate);
        wait_until(&mut app, player, on + 700);
        take_off(&mut app, player, plate);
        let again = app.world().resource::<Turns>().now();
        put_on(&mut app, player, plate);
        wait_until(&mut app, player, again + 700);
        assert_eq!(health(&app, player), 10, "the seven turns before it came off counted for nothing");
        wait_until(&mut app, player, again + 800);
        assert_eq!(health(&app, player), 11);
    }

    /// A wearer on a map that is not the current one has its pulse land on
    /// nobody: a pulse is landed by who stands on the wearer's cell, and on
    /// this map that is somebody else, standing at the same coordinates.
    #[test]
    fn a_pulse_on_a_wearer_elsewhere_lands_on_nobody_here() {
        let (mut app, player, _) = rig(None, None, MEND_ON_USE);
        let at = app.world().get::<Position>(player).unwrap().0;
        let pulsing = triggers_of(&app, MEND_ON_PULSE);
        let plate = app.world_mut().spawn((Item, pulsing, crate::items::Pulse::every(100))).id();
        let slot = rl_rules::SlotId::from_raw(0);
        let mut worn = rl_rules::Equipment::with_slot_count(1);
        worn.equip(plate, &rl_rules::EquipShape::in_slot(slot)).unwrap();
        // Elsewhere: on another map, at the player's own coordinates.
        app.world_mut().spawn((
            Actor,
            Position(at),
            crate::places::OnMap(crate::places::MapId(7)),
            Health::full(30),
            Inventory { items: vec![plate] },
            crate::items::Equipped(worn),
        ));
        wait_until(&mut app, player, 500);
        assert_eq!(health(&app, player), 10, "five of its pulses came round, and none of them landed on the player standing here");
    }
```

Run: `cargo test -p rl-bevy pulse`
Expected: FAIL to compile, `Pulse` not found.

- [ ] **Step 4: Add `Pulse` and its systems**

In `crates/rl-bevy/src/items.rs`, add `use crate::turn::Turns;` beside the other `crate::turn` import, then after `Stack`:

```rust
/// A worn thing's own clock: its `pulse` moment comes round every `every`
/// hundredths of a step it is worn, with `progress` counted towards the
/// next.
///
/// What a pulse does is the thing's `pulse` trigger, landed on whoever
/// stands on the wearer's cell, which is the wearer: a plate that knits
/// wounds is a pulse that mends. The clock runs only while the thing is
/// worn and starts again from nothing each time it is put on, so a wearer
/// cannot wear it most of a period, swap, and swap back for a pulse on the
/// next turn. The game writes `every` when it spawns the thing, with the
/// thing's enchant already applied, as it writes [`Bestows`].
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pulse {
    /// Hundredths of a step between pulses.
    pub every: u32,
    /// Hundredths counted towards the next.
    pub progress: u32,
}

impl Pulse {
    /// A clock coming round every `every` hundredths, with nothing counted yet.
    pub fn every(every: u32) -> Self {
        Self { every, progress: 0 }
    }
}
```

After `fold_gear`:

```rust
/// Counts each worn [`Pulse`] on by the time that passed since the last
/// pass, and reports its `pulse` moment for each full period, on the thing,
/// by its wearer, at the wearer's cell.
///
/// In [`ResolveSet::Triggers`](crate::plugin::ResolveSet::Triggers) before
/// the triggers land, so a pulse lands in the pass whose clock brought it.
/// Reads the clock rather than counting passes, as a recharge does, and a
/// clock that went backwards, a new run, counts as no time. Only wearers on
/// the current map: a moment lands on whoever stands on its cell here, and
/// a wearer on another map would mend a stranger at the same coordinates.
pub fn pulse_worn(
    turns: Res<Turns>,
    mut last: Local<Option<u32>>,
    map: Res<WorldMap>,
    wearers: Query<(Entity, &Position, Option<&OnMap>, &Equipped)>,
    mut pulses: Query<&mut Pulse>,
    mut fired: MessageWriter<crate::effects::Fired>,
) {
    let now = turns.now();
    let passed = now.saturating_sub(last.unwrap_or(now));
    *last = Some(now);
    if passed == 0 {
        return;
    }
    let here = map.current();
    for (wearer, pos, on, worn) in &wearers {
        if on.map_or(MapId::SURFACE, |m| m.0) != here {
            continue;
        }
        for (_, item) in worn.0.worn() {
            let Ok(mut pulse) = pulses.get_mut(item) else { continue };
            let every = pulse.every.max(1);
            pulse.progress += passed;
            while pulse.progress >= every {
                pulse.progress -= every;
                fired.write(crate::effects::Fired { on: item, moment: crate::effects::Moments::PULSE, by: Some(wearer), at: pos.0 });
            }
        }
    }
}

/// Starts a [`Pulse`] from nothing whenever its thing is put on.
///
/// In [`TurnSet::React`](crate::plugin::TurnSet::React), after the pass
/// counted whatever time it counted, so the thing's first period is a whole
/// one from the pass it went on in.
pub fn restart_pulses(mut events: MessageReader<ItemEvent>, mut pulses: Query<&mut Pulse>) {
    for ev in events.read() {
        let ItemEvent::Equipped { item, .. } = *ev else { continue };
        if let Ok(mut pulse) = pulses.get_mut(item) {
            pulse.progress = 0;
        }
    }
}
```

In `ItemsPlugin::build`, beside the other `add_systems`:

```rust
            // Before the triggers land, so a pulse lands in the pass that
            // brought it; a game with no effects has nothing to land it.
            .add_systems(Turn, pulse_worn.in_set(ResolveSet::Triggers).before(crate::effects::land_triggers))
            .add_systems(Turn, restart_pulses.in_set(TurnSet::React))
```

Add to the `ItemsPlugin` doc: "A worn [`Pulse`] reports its `pulse` moment on its own clock."
Export `Pulse` from `crates/rl-bevy/src/lib.rs` in the `pub use items::{...}` list and the prelude's `crate::items::{...}` list, alphabetically.

- [ ] **Step 5: Run the pulse tests**

Run: `cargo test -p rl-bevy pulse`
Expected: PASS. If the first test's "seven turns worn, not yet" fails with 11, the clock reached `on + 800` inside `wait_until(on + 700)`: print `Turns::now()` after each wait to see where; the rule is a mend once 800 hundredths have been worn, and the fix belongs in the test's arithmetic, never in `pulse_worn`.

- [ ] **Step 6: Save a pulse's progress**

Write the failing test in the tests module of `crates/rl-save/src/engine.rs`, beside the wand test it is shaped like:

```rust
    /// A worn thing's pulse comes back with the progress it had, since
    /// seven eighths of a mend is seven turns a player already spent. Its
    /// period comes from what the game respawned, as a wand's maximum does.
    #[test]
    fn a_pulse_comes_back_with_its_progress() {
        let (mut app, _) = fresh();
        let plate = app.world_mut().spawn(Pulse { every: 800, progress: 700 }).id();
        let mut remap = EntityRemap::new();
        let p_id = remap.save_id(plate);
        let save = {
            app.insert_resource(Seed(RunSeed(5)));
            EngineSave::capture(app.world_mut(), &mut remap)
        };
        let back: EngineSave = crate::decode(1, &crate::encode(1, &save).unwrap()).unwrap();

        let (mut app2, _) = fresh();
        let plate2 = app2.world_mut().spawn(Pulse::every(800)).id();
        let mut remap2 = EntityRemap::new();
        remap2.bind(p_id, plate2);
        back.restore(app2.world_mut(), &remap2);
        assert_eq!(app2.world().get::<Pulse>(plate2).map(|p| p.progress), Some(700));
    }
```

Run `cargo test -p rl-save a_pulse_comes_back` and see it fail to compile, then, once `Pulse` is imported below, with `Some(0)`.

Then in `EffectState`:

```rust
    /// Progress towards the next pulse, for a worn thing with a clock.
    #[serde(default)]
    pub pulse: Option<u32>,
```

In `EffectState::of`, read it and count it as worth saving:

```rust
        let pulse = world.get::<Pulse>(entity).map(|p| p.progress);
        ...
        (consumable.is_some() || pulse.is_some() || !fires.is_empty()).then_some(Self { consumable, pulse, fires })
```

In `EffectState::restore`, after the consumable:

```rust
        if let (Some(progress), Some(mut p)) = (self.pulse, target.get_mut::<Pulse>()) {
            p.progress = progress.min(p.every.saturating_sub(1));
        }
```

Import `Pulse` in the file's `use rl_bevy::{...}` line and update the `EffectState` doc's first paragraph to name "the progress towards a worn thing's next pulse". Run `cargo test -p rl-save` and see it pass.

- [ ] **Step 7: The bag names the pulse's period**

In `crates/rl-ui/src/view/inventory.rs`, change `Does` and `lead_in`:

```rust
/// What an item does at its moments, what a use costs it, and its clock.
type Does = (Option<&'static Triggers>, Option<&'static Consumable>, Option<&'static rl_bevy::Pulse>);
```

```rust
/// How a trigger's lines are introduced on a bag's row: by what the player
/// does to set it off, in the engine's own moments, and by the moment's
/// name for one a game registered. A pulse says how often it comes round.
fn lead_in(moment: MomentId, moments: Option<&Moments>, pulse: Option<&rl_bevy::Pulse>) -> String {
    match moment {
        m if m == Moments::USE => "use".to_string(),
        // Not "thrown": the row already says how far it flies, and the
        // word twice over reads as two things.
        m if m == Moments::LAND => "on landing".to_string(),
        m if m == Moments::HIT => "on a hit".to_string(),
        m if m == Moments::FIRE => "when fired".to_string(),
        m if m == Moments::PULSE => match pulse {
            Some(p) => format!("every {} turns worn", p.every.div_ceil(rl_core::turn::BASE_ACTION_COST)),
            None => "worn".to_string(),
        },
        m => moments.map(|all| all.name(m).to_string()).unwrap_or_default(),
    }
}
```

In `collect_inventory`, destructure `(triggers, consumable, pulse)` and call `lead_in(trigger.on, moments, pulse)`. Keep the existing doc comment text above `lead_in` that is not replaced.

Write a view test beside Task 2's, spawning a torso plate with a `Pulse::every(800)` and a `Triggers` holding one `Trigger { on: Moments::PULSE, .. }` whose effects are built from `Mend 1 care`; since the Stage has no `EffectKinds`, build the `Effects` by hand: `rl_bevy::Effects::build(&[spec], &kinds, &names)` needs kinds, so instead assert on `lead_in` directly:

```rust
    #[test]
    fn a_pulse_is_introduced_by_how_often_it_comes_round() {
        assert_eq!(lead_in(Moments::PULSE, None, Some(&rl_bevy::Pulse::every(800))), "every 8 turns worn");
        assert_eq!(lead_in(Moments::PULSE, None, Some(&rl_bevy::Pulse::every(750))), "every 8 turns worn", "a part turn rounds up, never promising early");
    }
```

Run: `cargo test -p rl-ui a_pulse_is_introduced` and see it pass.

- [ ] **Step 8: Docs**

`docs/design/items.md` section 6 bullet "**`on_equip` effects.**": after its first sentence add "A worn thing's clock is the other half, and it exists: a [`Pulse`] reports the `pulse` moment every period its thing is worn, so a plate that knits wounds is a trigger, not a system." Keep the rest.

`docs/design/effects.md` section 2: add a line: "`pulse` is the seventh built-in moment, a worn thing's clock coming round, appended so the six before it keep their ids."

Re-read `items.md`, `effects.md` and `saving.md` in `docs/guide/src/systems/`. In `items.md` `The model`, after the sentence about `Triggers`, add: "`Pulse` is a worn thing's own clock: `pulse_worn` reports the `pulse` moment every `every` hundredths it is worn, on the wearer's cell and only on the current map, and `restart_pulses` starts it from nothing each time it is put on." In `effects.md`, wherever the six moments are listed, list the seventh. In `saving.md`, where the saved state of a thing is listed, add the pulse's progress. Run `python3 scripts/check-systems.py`, re-read and bless each page it names, and `scripts/check-systems-style.sh` each.

`CHANGELOG.md` under `Unreleased`:

```markdown
- A worn thing can do something on its own clock: `Pulse { every, progress }` on an item reports the new built-in `pulse` moment every `every` hundredths of a step it is worn, on its wearer's cell, and starts from nothing each time it is put on; its `pulse` trigger says what it does. `Moments::BUILT_IN` has seven names, `pulse` last, so every earlier id is unchanged. `rl-save` keeps a pulse's progress.
```

- [ ] **Step 9: Commit**

```bash
git add crates docs CHANGELOG.md
git commit -m "a worn thing can pulse on its own clock while it is worn"
```

---

## Task 4: Attunement

**Files:**
- Modify: `crates/rl-bevy/src/consumable.rs` (module doc, `Attuned`, `recharge_charges`, `attune`, `ConsumablesPlugin::build`)
- Modify: `crates/rl-bevy/src/lib.rs:60` and the prelude's `consumable` line (export `Attuned`)
- Modify: `crates/rl-ui/src/view/inventory.rs` (`ItemRow::ready_in`, `ItemRow::attuned`, `collect_inventory`), `crates/rl-ui/src/panel/inventory.rs:301-345` (`describe`)
- Test: `crates/rl-bevy/src/consumable.rs` tests, `crates/rl-ui/src/panel/inventory.rs` tests
- Docs: `docs/guide/src/systems/items.md`, `effects.md` (bless), `docs/design/items.md` section 5, `CHANGELOG.md`

**Interfaces:**
- Consumes: Task 3's test helpers `a_slot_for`, `put_on`, `take_off`, `wait_until` in `consumable.rs` tests.
- Produces: `pub struct Attuned;` (component) in `rl_bevy::consumable`, re-exported; `pub fn attune(...)`; `ItemRow::ready_in: Option<u32>`, `ItemRow::attuned: bool`.

- [ ] **Step 1: Write the failing tests**

In the tests module of `crates/rl-bevy/src/consumable.rs`:

```rust
    /// An attuned thing is empty the moment it is put on and refills only
    /// while it is worn: forty turns in the bag buy nothing, and a charge
    /// comes back one period after it went on.
    #[test]
    fn an_attuned_thing_is_empty_when_put_on_and_charges_only_while_worn() {
        let (mut app, player, plate) = rig(Some(Consumable::new(1, WhenEmpty::Kept).recharging(400)), None, MEND_ON_USE);
        app.world_mut().entity_mut(plate).insert(Attuned);
        a_slot_for(&mut app, player, plate);
        let on = app.world().resource::<Turns>().now();
        put_on(&mut app, player, plate);
        assert_eq!(app.world().get::<Consumable>(plate).map(|c| c.left), Some(0), "full in the bag, empty once on");
        wait_until(&mut app, player, on + 300);
        assert_eq!(app.world().get::<Consumable>(plate).map(|c| c.left), Some(0), "three turns worn, still charging");
        wait_until(&mut app, player, on + 400);
        assert_eq!(app.world().get::<Consumable>(plate).map(|c| c.left), Some(1), "four turns worn, ready");
    }

    /// Put on, off, and on again: each putting-on empties it, and the turns
    /// it spent in the bag between refilled nothing.
    #[test]
    fn an_attuned_thing_taken_off_gains_nothing_and_is_empty_again_when_put_back_on() {
        let (mut app, player, plate) = rig(Some(Consumable::new(1, WhenEmpty::Kept).recharging(400)), None, MEND_ON_USE);
        app.world_mut().entity_mut(plate).insert(Attuned);
        a_slot_for(&mut app, player, plate);
        let on = app.world().resource::<Turns>().now();
        put_on(&mut app, player, plate);
        wait_until(&mut app, player, on + 300);
        take_off(&mut app, player, plate);
        let off = app.world().resource::<Turns>().now();
        wait_until(&mut app, player, off + 2000);
        let c = app.world().get::<Consumable>(plate).copied().unwrap();
        assert_eq!((c.left, c.recharge.map(|r| r.progress)), (0, Some(300)), "twenty turns off the body counted for nothing");
        put_on(&mut app, player, plate);
        let c = app.world().get::<Consumable>(plate).copied().unwrap();
        assert_eq!((c.left, c.recharge.map(|r| r.progress)), (0, Some(0)), "and back on, it starts over");
    }

    /// A thing that is not attuned refills in the bag as it always has.
    #[test]
    fn a_thing_that_is_not_attuned_refills_in_the_bag() {
        let empty = Consumable { left: 0, ..Consumable::new(1, WhenEmpty::Kept).recharging(400) };
        let (mut app, player, wand) = rig(Some(empty), None, MEND_ON_USE);
        wait_until(&mut app, player, 400);
        assert_eq!(app.world().get::<Consumable>(wand).map(|c| c.left), Some(1));
    }
```

Run: `cargo test -p rl-bevy attuned`
Expected: FAIL to compile, `Attuned` not found.

- [ ] **Step 2: Add `Attuned`, gate the recharge, empty on putting on**

In `crates/rl-bevy/src/consumable.rs`, after `Consumable`'s impl:

```rust
/// A worn thing whose charges come back only while it is worn, and which
/// is emptied each time it is put on.
///
/// The rule that makes swapping gear cost something: a plate that cloaks
/// its wearer cannot be carried charged and put on for the one turn it is
/// needed, nor kept charging in the bag while another plate is worn. What
/// it holds is earned by wearing it. Only a thing that can be worn means
/// anything by it.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Attuned;
```

Replace `recharge_charges`:

```rust
/// Counts every refilling consumable's progress up by the time that passed
/// since the last pass, and gives back a charge for each full period.
///
/// Reads the clock rather than counting passes, because a pass is one
/// actor's turn and the clock is what a refill is written in. A clock that
/// went backwards, a new run, counts as no time. An [`Attuned`] thing that
/// nobody is wearing counts nothing.
pub fn recharge_charges(
    turns: Res<Turns>,
    mut last: Local<Option<u32>>,
    mut things: Query<(Entity, &mut Consumable, Has<Attuned>)>,
    wearers: Query<&crate::items::Equipped>,
) {
    let now = turns.now();
    let passed = now.saturating_sub(last.unwrap_or(now));
    *last = Some(now);
    if passed == 0 {
        return;
    }
    let worn: Vec<Entity> = wearers.iter().flat_map(|w| w.0.worn().map(|(_, item)| item)).collect();
    for (thing, mut c, attuned) in &mut things {
        if attuned && !worn.contains(&thing) {
            continue;
        }
        let (left, max) = (c.left, c.max);
        let Some(r) = c.recharge.as_mut() else { continue };
        if left >= max {
            r.progress = 0;
            continue;
        }
        let every = r.every.max(1);
        r.progress += passed;
        let gained = (r.progress / every).min(u32::from(max - left)) as u16;
        r.progress %= every;
        let full = left + gained >= max;
        if full {
            r.progress = 0;
        }
        c.left = (left + gained).min(max);
    }
}

/// Empties an [`Attuned`] thing, charges and progress both, whenever it is
/// put on.
///
/// After [`recharge_charges`] in the same set, so the pass it went on in
/// counts nothing towards it.
pub fn attune(mut events: MessageReader<crate::items::ItemEvent>, mut things: Query<&mut Consumable, With<Attuned>>) {
    for ev in events.read() {
        let crate::items::ItemEvent::Equipped { item, .. } = *ev else { continue };
        let Ok(mut c) = things.get_mut(item) else { continue };
        c.left = 0;
        if let Some(r) = c.recharge.as_mut() {
            r.progress = 0;
        }
    }
}
```

In `ConsumablesPlugin::build`, replace the `recharge_charges` line:

```rust
            .add_systems(Turn, (recharge_charges, attune).chain().in_set(TurnSet::React))
```

Module doc: replace the paragraph starting "**What is not here.**" with:

```rust
//! **Worn things.** Wearing is declarative and needs no effects:
//! [`Armor`](crate::combat::Armor), [`Resists`](crate::combat::Resists), an
//! attack, [`Bestows`](crate::items::Bestows). What is here is the one rule
//! about charges and wearing: an [`Attuned`] thing refills only while it is
//! worn and is emptied each time it is put on, so a charge is earned by
//! wearing the thing rather than by carrying it.
```

Export `Attuned` (and `attune`, `recharge_charges` are already reachable through the module) from `lib.rs`'s `pub use consumable::{...}` and the prelude's list.

- [ ] **Step 3: Run the tests**

Run: `cargo test -p rl-bevy -- attuned refills_in_the_bag a_recharge_returns`
Expected: PASS, the existing recharge test included.

- [ ] **Step 4: The bag says when it will be ready**

In `crates/rl-ui/src/view/inventory.rs`, add to `ItemRow` after `empty`:

```rust
    /// Whole turns until the next charge comes back, for a thing that
    /// refills, is not full, and whose clock is running.
    pub ready_in: Option<u32>,
    /// Whether its charges come back only while it is worn.
    pub attuned: bool,
```

Extend `Does` with `Has<rl_bevy::Attuned>` as its fourth element, destructure it as `attuned`, and fill the two fields in `collect_inventory`:

```rust
        let clock_runs = !attuned || slot.is_some();
        let ready_in = consumable.filter(|c| c.left < c.max && clock_runs).and_then(|c| c.recharge).map(|r| (r.every - r.progress.min(r.every)).div_ceil(rl_core::turn::BASE_ACTION_COST));
```

and in the `ItemRow { .. }` literal `ready_in, attuned,`. Every other `ItemRow { .. }` literal in the crate's tests gains `ready_in: None, attuned: false`.

In `crates/rl-ui/src/panel/inventory.rs` `describe`, replace

```rust
    if row.empty {
        say("empty".to_string(), Tones::MUTED);
    }
```

with

```rust
    // An attuned thing off the body says why nothing is coming back; one
    // whose clock runs says when it will be ready; anything else empty is
    // simply empty.
    match (row.attuned && !row.worn(), row.empty, row.ready_in) {
        (true, _, _) => say("charges only while worn".to_string(), Tones::MUTED),
        (false, true, Some(turns)) => say(format!("ready in {turns} turns"), Tones::MUTED),
        (false, true, None) => say("empty".to_string(), Tones::MUTED),
        (false, false, _) => {}
    }
```

- [ ] **Step 5: Test the lines**

In the tests module of `crates/rl-ui/src/panel/inventory.rs`, beside `a_wand_counts_its_charges_and_an_empty_one_offers_no_use`, using the module's own `with_triggers` and `detail`:

```rust
    /// Two attuned plates, one worn and a quarter charged and one spare in
    /// the bag: the worn one says when it will be ready, and the spare says
    /// why it never will be while it stays there.
    #[test]
    fn a_charging_plate_says_when_it_is_ready_and_a_spare_says_it_charges_only_worn() {
        let (mut stage, triggers) = with_triggers(r#"[(on: "use", effects: [(kind: "Mend", args: (kind: "kinetic", roll: "2"))])]"#);
        stage.app.world_mut().resource_mut::<Registries>().slots = Registry::from_defs(vec![SlotDef::new("torso")]).unwrap();
        let player = stage.player;
        let torso = stage.app.world().resource::<Registries>().slots.expect("torso");
        let charging = Consumable { left: 0, recharge: Some(rl_bevy::Recharge { every: 4000, progress: 1000 }), ..Consumable::new(1, WhenEmpty::Kept) };
        let plate = |name: &str| (Item, Name::new(name.to_string()), triggers.clone(), charging, rl_bevy::Attuned, Wearable(EquipShape::in_slot(torso)));
        let worn_plate = stage.app.world_mut().spawn(plate("plate")).id();
        let spare = stage.app.world_mut().spawn(plate("spare")).id();
        let mut worn = Equipped(Equipment::with_slot_count(1));
        worn.equip(worn_plate, &EquipShape::in_slot(torso)).unwrap();
        stage.app.world_mut().entity_mut(player).insert((Inventory { items: vec![worn_plate, spare] }, worn));
        stage.tick();

        stage.press(KeyCode::KeyI);
        assert!(detail(&stage).iter().any(|l| l == "ready in 30 turns"), "{:?}", detail(&stage));
        stage.press(KeyCode::ArrowDown);
        assert!(detail(&stage).iter().any(|l| l == "charges only while worn"), "{:?}", detail(&stage));
        assert!(!detail(&stage).iter().any(|l| l == "empty"), "the reason, not the bare fact: {:?}", detail(&stage));
    }
```

The Stage adds no `ConsumablesPlugin`, so nothing recharges while the test ticks. Import `Consumable`, `WhenEmpty`, `Wearable`, `Equipped` and `Item` from `rl_bevy::prelude` if the module's `use super::*` does not already bring them. Run `cargo test -p rl-ui a_charging_plate_says` and see it pass.

- [ ] **Step 6: Docs**

`docs/design/items.md` section 5, after the paragraph on `Recharge`, add:

```markdown
A worn thing that holds charges may be `Attuned`: it refills only while worn and is emptied each time it is put on.
That is the anti-swap rule, and it is on the thing rather than on the wearer because it is a fact about the thing: a plate that cloaks its wearer is earned by wearing it.
Equip time was the other candidate, and it was rejected as the rule because time alone still lets a player swap between fights.
A pulse is attuned by nature, since its clock only runs while worn.
```

Re-read `items.md` and `effects.md` in the guide; in `items.md` `The model` after the sentence on `recharge_charges`, add: "An `Attuned` thing refills only while worn, and `attune` empties it each time it is put on." and mention the two new bag lines where the bag's row is described. Check, bless and style-check every page `check-systems.py` names.

`CHANGELOG.md`:

```markdown
- `Attuned` on a worn consumable makes its charges come back only while it is worn and empties it each time it is put on, so a charge is earned by wearing the thing. `ItemRow` gains `ready_in` and `attuned`, and the bag says "ready in N turns" or "charges only while worn" where it said "empty"; a game that builds an `ItemRow` by hand adds the two fields.
```

- [ ] **Step 7: Commit**

```bash
git add crates docs CHANGELOG.md
git commit -m "an attuned thing refills only while worn and starts empty when put on"
```

---

## Task 5: Effects land at a level

**Files:**
- Modify: `crates/rl-bevy/src/effects/mod.rs:94-117` (`Landing`), `:236-247` (`Effect::describe`), `:388-411` (`Effects::describe`, `land_on`)
- Modify: `crates/rl-bevy/src/effects/engine.rs` (every `describe`; `per_level` on `Harm`, `Mend`, `Inflict`)
- Modify: `crates/rl-bevy/src/effects/triggers.rs:280-310` (`land_triggers`)
- Modify: `crates/rl-bevy/src/ability.rs:261-263`, `:385`, `:886-900` (test `Mark`)
- Modify: `crates/rl-ui/src/view/inventory.rs` (`Does` gains `Option<&Enchant>`; describe at the item's level)
- Modify: `crates/rl-bevy/tests/genres.rs`, `examples/corsair/src/abilities.rs:60-63`, `examples/delve/src/effects.rs:54-56`
- Test: `crates/rl-bevy/src/effects/engine.rs` (new tests module), `crates/rl-bevy/src/consumable.rs` tests
- Docs: `docs/guide/src/systems/effects.md`, `abilities.md`, `items.md`, `props.md`, `fields.md` (bless), `docs/design/effects.md`, `CHANGELOG.md`

**Interfaces:**
- Produces:
  - `Landing::level: i32`.
  - `Effect::describe(&self, registries: &Registries, level: i32) -> String`.
  - `Effects::describe(&self, registries: &Registries, level: i32) -> Vec<String>`.
  - `Harm::per_level: i32`, `Harm::roll_at(level: i32) -> DiceRoll`; the same on `Mend`; `Inflict::per_level: u32`, `Inflict::turns_at(level: i32) -> u32`.
  - RON: `(kind: "Inflict", args: (status: "cloaked", turns: 10, per_level: 2))`, `(kind: "Mend", args: (kind: "care", roll: "1", per_level: 1))`.

- [ ] **Step 1: Write the failing pure tests**

At the end of `crates/rl-bevy/src/effects/engine.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// What a level adds: a point on the roll per level for harm and a
    /// mend, a turn per level for a status, and nothing at all below `+1`.
    #[test]
    fn each_level_adds_its_per_level_and_a_plain_thing_adds_nothing() {
        let kind = DamageKindId::from_raw(0);
        let mend = Mend { kind, roll: DiceRoll::flat(1), per_level: 2 };
        assert_eq!((mend.roll_at(0), mend.roll_at(3)), (DiceRoll::flat(1), DiceRoll::flat(7)));
        let harm = Harm { kind, roll: DiceRoll::new(2, 6), per_level: 1 };
        assert_eq!(harm.roll_at(2), DiceRoll { num: 2, sides: 6, bonus: 2 });
        let cloak = Inflict { status: StatusId::from_raw(0), turns: 5, per_level: 1 };
        assert_eq!((cloak.turns_at(0), cloak.turns_at(2), cloak.turns_at(-1)), (5, 7, 5), "a negative level is plain, never shorter");
    }

    /// The arguments read `per_level` when it is written and nought when
    /// it is not, so every content file written before this still loads.
    #[test]
    fn per_level_is_read_when_written_and_nought_when_not() {
        let kinds = rl_rules::Registry::from_defs(vec![rl_rules::DamageKind::new("care")]).unwrap();
        let statuses = rl_rules::Registry::from_defs(vec![rl_rules::StatusDef::new("cloaked")]).unwrap();
        let names = Names::new().damage_kinds(&kinds).statuses(&statuses);
        let args = |text: &str| rl_rules::ability::parse_args(text).unwrap();
        let old = Mend::from_args(&args(r#"(kind: "care", roll: "4")"#), &names).unwrap();
        assert_eq!(old.per_level, 0);
        let new = Inflict::from_args(&args(r#"(status: "cloaked", turns: 5, per_level: 1)"#), &names).unwrap();
        assert_eq!(new.per_level, 1);
    }
}
```

Check `Names` has `.statuses(&registry)`; if its builder method is named otherwise (read `crates/rl-rules/src/names.rs`), use that name.

Run: `cargo test -p rl-bevy each_level_adds`
Expected: FAIL to compile, no field `per_level`.

- [ ] **Step 2: Add `per_level` to the three effects**

In `crates/rl-bevy/src/effects/engine.rs`. For `Harm` (and identically for `Mend`, with its own wording):

```rust
#[derive(Debug, Clone, Copy)]
pub struct Harm {
    /// What kind of damage.
    pub kind: DamageKindId,
    /// How much, rolled per target, at level zero.
    pub roll: DiceRoll,
    /// Added to the roll for each enchant level of what landed it.
    pub per_level: i32,
}

impl Harm {
    /// The roll at `level`; a level below one adds nothing.
    pub fn roll_at(&self, level: i32) -> DiceRoll {
        DiceRoll { bonus: self.roll.bonus + self.per_level * level.max(0), ..self.roll }
    }
}

impl Effect for Harm {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let roll = self.roll_at(landing.level);
        for target in &landing.targets {
            let amount = roll.roll_at_least(&mut **world.rng, 0);
            world.damage.write(DamageEvent::new(*target, Hit::by(landing.user, self.kind, amount)));
        }
    }

    fn describe(&self, registries: &Registries, level: i32) -> String {
        format!("{} {}", self.roll_at(level), registries.damage_kinds.name(self.kind))
    }
}

impl FromArgs for Harm {
    const KIND: &'static str = "Harm";

    fn from_args(args: &RawValue, names: &Names<'_>) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        struct Args {
            kind: String,
            roll: String,
            #[serde(default)]
            per_level: i32,
        }
        let a: Args = read_args(args)?;
        Ok(Self { kind: names.damage_kind(&a.kind)?, roll: a.roll.parse().map_err(|e| format!("{e}"))?, per_level: a.per_level })
    }
}
```

`Mend` is the same shape: `roll_at`, `apply` using `self.roll_at(landing.level)`, and `describe` returning `format!("mends {} {}", self.roll_at(level), ...)`.

For `Inflict`:

```rust
#[derive(Debug, Clone, Copy)]
pub struct Inflict {
    /// Which status.
    pub status: StatusId,
    /// For how many whole turns, at level zero.
    pub turns: u32,
    /// Turns added for each enchant level of what landed it.
    pub per_level: u32,
}

impl Inflict {
    /// The turns at `level`; a level below one adds nothing.
    pub fn turns_at(&self, level: i32) -> u32 {
        self.turns + self.per_level * level.max(0) as u32
    }
}
```

with `apply` writing `turns: self.turns_at(landing.level)`, `describe` returning `format!("{} for {} turns", registries.statuses.name(self.status), self.turns_at(level))`, and `Args` gaining `#[serde(default)] per_level: u32`.

Every other effect's `describe` in the file takes `_: i32` as a third parameter and ignores it.

Update the module doc's first paragraph by adding: "`Harm`, `Mend` and `Inflict` grow with the enchant level of what landed them, through `per_level`; the others do the same thing at any level."

- [ ] **Step 3: `Landing::level` and `describe` with a level**

In `crates/rl-bevy/src/effects/mod.rs`, add to `Landing` after `targets`:

```rust
    /// The enchant level of what landed it: a worn or thrown thing's
    /// [`Enchant`](crate::items::Enchant), and nought for an ability, an
    /// offer, or anything plain. An effect that grows with it reads it here.
    pub level: i32,
```

Change the trait method:

```rust
    /// What this does at `level`, in a few words for a menu, with every id
    /// named through `registries`: `3d6 fire`, `scorched for 4 turns`. Empty
    /// means the menu says nothing about it, which is the default so an
    /// effect a game writes in a hurry still loads.
    fn describe(&self, registries: &crate::registries::Registries, level: i32) -> String {
        let _ = (registries, level);
        String::new()
    }
```

`Effects::describe(&self, registries: &Registries, level: i32)` passes `level` to each `b.effect.describe(registries, level)`; update its doc to say "at `level`". `land_on` builds its `Landing` with `level: 0`.

In `crates/rl-bevy/src/ability.rs`: the `Landing { .. }` at line 385 gains `level: 0`, and `Abilities::describe` calls `self.built[id.index()].describe(registries, 0)`.

In `crates/rl-bevy/src/effects/triggers.rs` `land_triggers`, read the carrier's level:

```rust
    mut carriers: Query<(&mut Triggers, Has<Remnant>, Has<LandsAsItself>, Option<&crate::items::Enchant>)>,
```

```rust
        let Ok((mut triggers, remnant, itself, enchant)) = carriers.get_mut(f.on) else { continue };
        let level = enchant.map_or(0, |e| e.level);
```

and `level,` in the `Landing { .. }` literal. Add to `land_triggers`' doc: "An enchanted carrier lands its effects at its level."

- [ ] **Step 4: Every game effect takes the level**

- `crates/rl-bevy/tests/genres.rs`: the macro's `impl Effect` needs no change (it uses the default `describe`); build and see.
- `crates/rl-bevy/src/ability.rs` test `Mark`: no `describe`, no change.
- `examples/corsair/src/abilities.rs:61`: `fn describe(&self, _: &Registries, _: i32) -> String`.
- `examples/delve/src/effects.rs:54`: `fn describe(&self, registries: &Registries, _: i32) -> String`.

- [ ] **Step 5: The bag describes a thing at its level**

In `crates/rl-ui/src/view/inventory.rs`, add `Option<&'static Enchant>` as the last element of `Does`, destructure it as `enchant`, and call `trigger.effects.describe(registries, enchant.map_or(0, |e| e.level))`.

- [ ] **Step 6: Write and run the landing test**

In the tests module of `crates/rl-bevy/src/consumable.rs`:

```rust
    /// A thing enchanted to `+3` lands its effects at its level: a mend of
    /// four with two a level mends ten.
    #[test]
    fn an_enchanted_thing_lands_its_effects_at_its_level() {
        let mends = r#"[(on: "use", effects: [(kind: "Mend", args: (kind: "care", roll: "4", per_level: 2))])]"#;
        let (mut app, player, item) = rig(None, None, mends);
        app.world_mut().entity_mut(item).insert(crate::items::Enchant(rl_rules::Enchanted { level: 3, affixes: Vec::new() }));
        use_it(&mut app, player, item);
        assert_eq!(health(&app, player), 20, "four and two for each of three levels");
    }
```

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 7: Docs**

`docs/design/effects.md`: add a section before "The roads not taken" (renumber the sections after it):

```markdown
## Effects at a level

An enchanted thing's effects grow with its level, and the level reaches them on the `Landing`.
`land_triggers` reads it off the carrier's `Enchant`; an ability and an offer land at nought.
`Harm` and `Mend` add `per_level` to the roll's bonus per level and `Inflict` adds `per_level` turns, each nought unless written, so every file written before this loads unchanged.
`describe` takes the level too, so the bag says what a `+2` thing does at `+2`.

The level is on the landing rather than baked into the effects at spawn because the effects are built once per definition and shared by every copy behind an `Arc`, and because the arguments are text the engine does not understand: rewriting them per level would silently skip a field that is not a number.
What is a number on the thing rather than in an effect, a pulse's period or a plate's armor, is written by the game at spawn with the level applied, as `Bestows` already is.
```

Re-read the guide pages `check-systems.py` names (`effects.md`, `abilities.md`, `items.md`, `props.md`, `fields.md`, and any other), correct each where it quotes `describe` or lists an effect's arguments (`effects.md` lists `Harm`, `Mend` and `Inflict`'s arguments: add `per_level`), then bless and style-check each.

`CHANGELOG.md`:

```markdown
- An effect lands at the enchant level of what landed it: `Landing::level`, read off the carrier's `Enchant` by `land_triggers` and nought for an ability or an offer. `Harm` and `Mend` take `per_level`, added to the roll per level, and `Inflict` takes `per_level`, turns added per level; both default to nought. Breaking: `Effect::describe` and `Effects::describe` take the level as a last argument, so a game's own effect that overrides `describe` adds `_: i32`, and a `Landing` built by hand adds `level: 0`.
```

- [ ] **Step 8: Commit**

```bash
git add crates examples docs CHANGELOG.md
git commit -m "an effect lands at the enchant level of what landed it"
```

---

## Task 6: The band reaches the maker

**Files:**
- Modify: `crates/rl-bevy/src/loot.rs` (`Provenance`, `ItemMaker::make`, `lay`, `scatter_places`, `scatter_regions`, `drop_on_death`, `fill_containers`, tests' `Toys` and `FoundAs`)
- Modify: `crates/rl-bevy/src/prefabs.rs:168-230` (`Drawn::Items`, `fill_prefabs`)
- Modify: `crates/rl-bevy/src/lib.rs:79` and the prelude's `loot` line (export `Provenance`)
- Modify: `examples/foundry/src/gear.rs:391-396`, `examples/corsair/src/items.rs:344-349`
- Test: `crates/rl-bevy/src/loot.rs` tests
- Docs: `docs/guide/src/systems/loot.md`, `prefabs.md` (bless), `docs/design/loot.md`, `CHANGELOG.md`

**Interfaces:**
- Produces: `pub struct Provenance { pub found: Found, pub band: i32 }`; `ItemMaker::make(&self, commands: &mut Commands, registries: &Registries, def: Id<Self::Def>, count: u32, from: Provenance, rng: &mut StdRng) -> Vec<Entity>`. Task 9's `Armory::make` reads `from.band`.

- [ ] **Step 1: Write the failing tests**

In the tests module of `crates/rl-bevy/src/loot.rs`, change `FoundAs` to hold a `Provenance`:

```rust
    /// How a made thing was found and at what band, so a test can see what
    /// the engine said.
    #[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct FoundAs(pub(crate) Provenance);
```

and `Toys::make`'s signature to `from: Provenance`, spawning `FoundAs(from)`. Every existing assertion `f.0 == Found::X` becomes `f.0.found == Found::X`. Add to `a_container_draws_its_kinds_at_its_band_and_its_offset`, inside `contents`, after the existing `Found::Container` assertion:

```rust
            let bands: Vec<i32> = held.iter().filter_map(|i| world.get::<FoundAs>(*i)).map(|f| f.0.band).collect();
            assert!(bands.contains(&(map.0 as i32 + 2)), "the weapon row was made at the container's band plus its offset: {bands:?}");
            assert!(bands.contains(&(map.0 as i32)), "and the armor at the container's own: {bands:?}");
```

In `the_dead_leave_what_their_drops_roll_where_they_fell`:

```rust
        assert!(world.query::<&FoundAs>().iter(world).all(|f| f.0 == Provenance { found: Found::Drop, band: 2 }), "made at the band of the place it died in");
```

replacing its last assertion. In the scatter test that checks `Found::Scatter` (line ~595), assert the band equals the place's map number as well: read that test first and add `&& f.0.band == <that test's map number>`.

Run: `cargo test -p rl-bevy loot`
Expected: FAIL to compile, `Provenance` not found.

- [ ] **Step 2: Add `Provenance` and pass it everywhere**

In `crates/rl-bevy/src/loot.rs`, after `Found`:

```rust
/// Why something is being made, and at what band: what a game rolls a
/// thing's quality or enchant from.
///
/// The band is the one the draw was asked at: a place's or a region's for
/// its floor, a container's plus its row's offset, a prefab slot's plus
/// its own, and for a drop the band of the place the actor died in. Two
/// facts beside each other rather than a band inside [`Found`], because a
/// game compares `Found` as a plain value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Provenance {
    /// Why it is being made.
    pub found: Found,
    /// How deep, far or dangerous where it will be found is.
    pub band: i32,
}
```

Change `ItemMaker::make` to take `from: Provenance` in place of `found: Found`, and its doc's last sentence to "`from` says why it is being made and at what band, and `rng` is the engine's stream for what is being made, for whatever the game rolls on the thing itself, a quality or an enchant." Change the module doc's "Whatever is made is told why, as [`Found`]" to "Whatever is made is told why and at what band, as [`Provenance`]".

`lay` takes `from: Provenance` in place of `found: Found` and passes it to `make`. Callers:

- `scatter_places`: `Provenance { found: Found::Scatter, band }`.
- `scatter_regions`: the same.
- `fill_containers`: `Provenance { found: Found::Container, band }`.
- `drop_on_death`: add `world: Option<Res<WorldRes>>` to its parameters and, per death:

```rust
        let map = on.map_or(MapId::SURFACE, |m| m.0);
        let band = maker.band(area_of(map, death.at, world.as_deref()));
        for (def, count) in drops.0.roll(&mut rng.0) {
            for item in maker.make(&mut commands, &registries, def, count, Provenance { found: Found::Drop, band }, &mut rng.0) {
```

In `crates/rl-bevy/src/prefabs.rs`, `Drawn::Items` carries its band:

```rust
    Items(Vec<(Id<I>, u32)>, i32),
```

its construction becomes `Drawn::Items(draw_stock(&**items, &roll.what, count, band, &mut rng), band)`, the lay arm `Drawn::Items(made, band) if !made.is_empty()` calls `lay(..., Provenance { found: Found::Placed, band }, &mut rng)`, and the fall-through arm is `Drawn::Items(..) | Drawn::Nothing`.

Games:

- `examples/foundry/src/gear.rs`: `fn make(&self, commands: &mut Commands, registries: &Registries, def: Id<ItemDef>, count: u32, _: Provenance, _: &mut rand::rngs::StdRng) -> Vec<Entity>`, importing `Provenance` from `rl_engine::rl_bevy` beside `Found`, and dropping `Found` from the import if unused.
- `examples/corsair/src/items.rs`: the same signature with `_: Provenance`; its body is unchanged.

Export `Provenance` from `lib.rs`'s `pub use loot::{...}` and the prelude.

- [ ] **Step 3: Run the tests**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 4: Docs**

`docs/design/loot.md`: in the bullet that says "Whatever the game rolls on the thing it makes, a quality or an enchant, comes from the stream the engine hands `make`", add: "and at the band the engine hands it, in `Provenance` beside why it is being made, so a crate two bands down stocks better-enchanted things than one on the floor above it."

Re-read `loot.md` and `prefabs.md` in the guide; wherever `make` or `Found` is described, say `Provenance { found, band }` and what band each source passes. Check, bless, style-check.

`CHANGELOG.md`:

```markdown
- `ItemMaker::make` is told the band it is making at: its `found: Found` argument is `from: Provenance`, `Provenance { found, band }`, the band being the place's or region's for a floor, a container's plus its row's offset, a prefab slot's plus its own, and the place's a drop fell in. Breaking: a game's `make` takes `from: Provenance` and reads `from.found` where it read `found`.
```

- [ ] **Step 5: Commit**

```bash
git add crates examples docs CHANGELOG.md
git commit -m "the maker is told the band it is making things at"
```

---

## Task 7: Levels by band

**Files:**
- Modify: `crates/rl-rules/src/loot.rs` (`LevelRow`, `LevelTable`, `load_levels`, module doc)
- Modify: `crates/rl-rules/src/lib.rs:22-24` (module list doc), `:91`, `:119` (exports)
- Test: `crates/rl-rules/src/loot.rs` tests
- Docs: `docs/guide/src/systems/loot.md` (bless), `docs/design/loot.md`, `CHANGELOG.md`

**Interfaces:**
- Produces, in `rl_rules::loot` and re-exported from `rl_rules`:
  - `pub struct LevelRow { pub bands: (i32, i32), pub levels: Vec<(i32, u32)> }`
  - `pub struct LevelTable` with `LevelTable::new(rows: Vec<LevelRow>) -> Result<LevelTable, Vec<String>>`, `rows(&self) -> &[LevelRow]`, `row_for(&self, band: i32) -> Option<&LevelRow>`, `roll(&self, band: i32, rng: &mut impl Rng) -> i32`; `Default` is the empty table, which rolls nought.
  - `pub fn load_levels(text: &str) -> Result<LevelTable, ContentError>`.

- [ ] **Step 1: Write the failing tests**

In the tests module of `crates/rl-rules/src/loot.rs`:

```rust
    fn levels() -> LevelTable {
        LevelTable::new(vec![
            LevelRow { bands: (1, 2), levels: vec![(0, 1)] },
            LevelRow { bands: (3, 5), levels: vec![(0, 3), (1, 1)] },
            LevelRow { bands: (9, 10), levels: vec![(2, 1), (3, 1)] },
        ])
        .expect("a sound table")
    }

    #[test]
    fn a_band_rolls_from_its_row_and_one_past_the_table_from_the_nearest() {
        let t = levels();
        assert_eq!(t.row_for(4).map(|r| r.bands), Some((3, 5)));
        assert_eq!(t.row_for(7).map(|r| r.bands), Some((3, 5)), "a gap falls back to the nearest shallower row");
        assert_eq!(t.row_for(12).map(|r| r.bands), Some((9, 10)), "past the table, the deepest row");
        assert_eq!(t.row_for(-3).map(|r| r.bands), Some((1, 2)), "before it, the shallowest");
        assert_eq!(LevelTable::default().row_for(4), None);
    }

    #[test]
    fn levels_are_drawn_by_weight_over_a_span_of_seeds() {
        let t = levels();
        let mut rng = rand::rngs::StdRng::seed_from_u64(3);
        let ones = (0..400).filter(|_| t.roll(4, &mut rng) == 1).count();
        assert!((60..=140).contains(&ones), "one in four at weight three to one: {ones} of 400");
        assert!((0..200).all(|_| (2..=3).contains(&t.roll(20, &mut rng))), "past the table, the deepest row's levels");
    }

    #[test]
    fn a_row_with_one_level_draws_nothing_from_the_stream() {
        let t = levels();
        let (mut a, mut b) = (rand::rngs::StdRng::seed_from_u64(9), rand::rngs::StdRng::seed_from_u64(9));
        assert_eq!(t.roll(1, &mut a), 0);
        assert_eq!(LevelTable::default().roll(1, &mut a), 0);
        assert_eq!(a.random::<u64>(), b.random::<u64>(), "the stream is where it was");
    }

    #[test]
    fn a_table_with_overlapping_bands_or_empty_rows_is_refused_with_every_problem() {
        let errors = LevelTable::new(vec![
            LevelRow { bands: (1, 4), levels: vec![(0, 1)] },
            LevelRow { bands: (3, 6), levels: vec![] },
            LevelRow { bands: (8, 7), levels: vec![(-1, 0)] },
        ])
        .expect_err("five problems");
        assert_eq!(errors.len(), 5, "an overlap, an empty row, a backwards range, a negative level, a weight of nothing: {errors:#?}");
    }

    #[test]
    fn a_level_file_loads_the_table_it_writes() {
        let t = load_levels("[(bands: (1, 2), levels: [(0, 1)]), (bands: (3, 5), levels: [(0, 3), (1, 1)])]").expect("it loads");
        assert_eq!(t.rows().len(), 2);
        assert_eq!(t.rows()[1].levels, vec![(0, 3), (1, 1)]);
    }
```

The test module already imports `rand::SeedableRng` if other tests seed; add `use rand::{Rng, SeedableRng};` if it does not.

Run: `cargo test -p rl-rules level`
Expected: FAIL to compile.

- [ ] **Step 2: Implement**

In `crates/rl-rules/src/loot.rs`, after `DropTable` and before `ScatterRules`:

```rust
/// One row of a level table: the bands it covers, both included, and the
/// levels a thing found there may have, each with its weight.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LevelRow {
    /// The shallowest and deepest band it covers.
    pub bands: (i32, i32),
    /// Each level and how often it is drawn against the others.
    pub levels: Vec<(i32, u32)>,
}

/// How good a thing found at a band is: an enchant level drawn by weight
/// from the row covering the band.
///
/// The engine owns the arithmetic and the game owns the rest: which things
/// are enchantable, the most any one of them reaches, and what a level
/// does to it. Rows never share a band, since two answers for one depth is
/// a typo rather than a choice. A band no row covers is read from the
/// nearest shallower row, else the nearest deeper one, as a tagged loot
/// draw falls back, so a crate two bands past the deepest row is as good
/// as the deepest row and never plain. A row with one level draws nothing,
/// so a table whose shallow bands are all `+0` moves no stream there.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LevelTable {
    rows: Vec<LevelRow>,
}

impl LevelTable {
    /// A table of `rows`, sorted by band, or every problem with them.
    pub fn new(mut rows: Vec<LevelRow>) -> Result<Self, Vec<String>> {
        let mut errors = Vec::new();
        for (i, r) in rows.iter().enumerate() {
            let at = format!("row {}", i + 1);
            if r.bands.0 > r.bands.1 {
                errors.push(format!("{at}: bands {} to {} is no range at all", r.bands.0, r.bands.1));
            }
            if r.levels.is_empty() {
                errors.push(format!("{at}: no levels to draw"));
            }
            for (level, weight) in &r.levels {
                if *level < 0 {
                    errors.push(format!("{at}: level {level} is below plain"));
                }
                if *weight == 0 {
                    errors.push(format!("{at}: level {level} has a weight of nothing; leave it out instead"));
                }
            }
        }
        rows.sort_by_key(|r| r.bands);
        for pair in rows.windows(2) {
            if pair[1].bands.0 <= pair[0].bands.1 {
                errors.push(format!("bands {:?} and {:?} overlap", pair[0].bands, pair[1].bands));
            }
        }
        if errors.is_empty() { Ok(Self { rows }) } else { Err(errors) }
    }

    /// The rows, shallowest first.
    pub fn rows(&self) -> &[LevelRow] {
        &self.rows
    }

    /// The row a draw at `band` is made from.
    pub fn row_for(&self, band: i32) -> Option<&LevelRow> {
        let covers = |r: &&LevelRow| (r.bands.0..=r.bands.1).contains(&band);
        self.rows
            .iter()
            .find(covers)
            .or_else(|| self.rows.iter().rev().find(|r| r.bands.1 < band))
            .or_else(|| self.rows.iter().find(|r| r.bands.0 > band))
    }

    /// A level for a thing found at `band`; plain for an empty table.
    pub fn roll(&self, band: i32, rng: &mut impl Rng) -> i32 {
        let Some(row) = self.row_for(band) else { return 0 };
        if let [(level, _)] = row.levels.as_slice() {
            return *level;
        }
        let total: u32 = row.levels.iter().map(|(_, w)| w).sum();
        let mut pick = rng.random_range(0..total);
        for (level, weight) in &row.levels {
            if pick < *weight {
                return *level;
            }
            pick -= weight;
        }
        0
    }
}

/// Loads a level table from RON, a list of rows.
///
/// Every field of a row:
///
/// - `bands`: `(shallowest, deepest)`, both included; no two rows share a band.
/// - `levels`: `[(level, weight)]`, each level at or above nought and each
///   weight above nought; only the ratio of the weights matters.
///
/// Every problem in the file is reported at once.
///
/// ```
/// use rl_rules::loot;
/// use rand::SeedableRng;
///
/// let table = loot::load_levels("[(bands: (1, 3), levels: [(0, 1)]), (bands: (4, 9), levels: [(1, 1), (2, 1)])]").unwrap();
/// let mut rng = rand::rngs::StdRng::seed_from_u64(1);
/// assert_eq!(table.roll(2, &mut rng), 0);
/// assert!((1..=2).contains(&table.roll(12, &mut rng)), "past the table, its deepest row");
/// ```
pub fn load_levels(text: &str) -> Result<LevelTable, ContentError> {
    let rows: Vec<LevelRow> = ron::from_str(text).map_err(|e| ContentError::Parse(e.to_string()))?;
    LevelTable::new(rows).map_err(ContentError::Invalid)
}
```

If `ron` or `Rng` is not already imported in the file, import them as the file's other loaders do (read its `use` lines; `Registry::from_ron_str` shows how this crate parses RON, and a `ContentError::Parse` variant exists as `ability.rs`' `parse_args` uses it). Add a line to the module doc's list: "- [`LevelTable`], how good a thing found at a band is, loaded with [`load_levels`]." Export `LevelRow`, `LevelTable` from `crates/rl-rules/src/lib.rs` in both `pub use loot::{...}` lists.

- [ ] **Step 3: Run the tests and the wasm build**

Run: `cargo test -p rl-rules` then `scripts/check-tiers.sh --wasm`
Expected: PASS, doc-test included.

- [ ] **Step 4: Docs**

`docs/design/loot.md`: add a section before the rejected alternatives:

```markdown
## Levels by band

How good a found thing is follows the band the way what it is does, and the arithmetic is the engine's: `LevelTable` holds rows of `(bands, [(level, weight)])`, rolls a level at a band, and falls back to the nearest row past either end, as a tagged draw does.
What is not the engine's is which things take a level and how far: a game consults the table in its own `make`, from the stream and at the band `Provenance` hands it, for the things it knows are enchantable, and caps the draw at what each allows.
A row with one level draws nothing, so the shallow decks of a game whose early finds are all plain move no stream.
```

Re-read `docs/guide/src/systems/loot.md`: in `The model` add "`LevelTable`, loaded with `loot::load_levels`, rolls an enchant level by band for a game's `make` to put on the things it knows are enchantable." Check, bless, style-check.

`CHANGELOG.md`:

```markdown
- `rl_rules::loot::LevelTable`, loaded with `loot::load_levels`, rolls an enchant level for a thing found at a band from rows of `(bands, [(level, weight)])`, falling back to the nearest row past either end and drawing nothing where a row has one level. A game's `ItemMaker::make` consults it at `from.band`.
```

- [ ] **Step 5: Commit**

```bash
git add crates docs CHANGELOG.md
git commit -m "a level table rolls how good a thing found at a band is"
```

---

## Task 8: The unseen

**Files:**
- Modify: `crates/rl-rules/src/status.rs` (`StatusDef::unseen`, `StatusDef::unseen()`, `Authored`, `load` and its doc)
- Modify: `crates/rl-bevy/src/stealth.rs` (module doc, `Unseen`, `mark_unseen`, `reveal_attackers`, `filter_unseen`, `Watchers`, `update_awareness`, `StealthPlugin::build`)
- Modify: `crates/rl-bevy/src/lib.rs:104` and the prelude's `stealth` line (export `Unseen`)
- Test: `crates/rl-rules/src/status.rs` tests, `crates/rl-bevy/src/stealth.rs` tests
- Docs: `docs/guide/src/systems/stealth.md`, `statuses.md` and whatever else `check-systems.py` names (bless), `docs/design/stealth.md`, `README.md`, `CHANGELOG.md`

**Interfaces:**
- Produces: `StatusDef::unseen: bool`; `StatusDef::unseen(self) -> Self`; `pub struct Unseen;` (component, `rl_bevy::stealth::Unseen`, re-exported); `pub fn mark_unseen`, `pub fn reveal_attackers`, `pub fn filter_unseen`. Task 9's `cloaked` status and faded commando rely on `Unseen`.

- [ ] **Step 1: The status property, test first**

In the tests module of `crates/rl-rules/src/status.rs` (read how existing tests build `Names` for `load` first):

```rust
    #[test]
    fn a_status_can_be_written_unseen_and_is_seen_unless_it_says_so() {
        let names = Names::new();
        let loaded = load(r#"[(name: "cloaked", unseen: true), (name: "dazed")]"#, &names).expect("it loads");
        assert!(loaded.get(loaded.expect("cloaked")).unseen);
        assert!(!loaded.get(loaded.expect("dazed")).unseen);
        assert!(StatusDef::new("cloaked").unseen().unseen);
    }
```

Run: `cargo test -p rl-rules a_status_can_be_written_unseen` and see it fail to compile.

Add to `StatusDef` after `badge`:

```rust
    /// Whether nothing sees whoever holds it while it lasts. What that
    /// means is stealth's to say: no mind perceives the holder and no
    /// observer notices it, and an attack it makes ends it.
    pub unseen: bool,
```

`StatusDef::new` sets `unseen: false`; add the builder:

```rust
    /// Makes whoever holds it unseen while it lasts.
    pub fn unseen(mut self) -> Self {
        self.unseen = true;
        self
    }
```

`Authored` gains `#[serde(default)] unseen: bool`, `load` passes `unseen: a.unseen`, and `load`'s doc list gains:

```rust
/// - `unseen`: `true` for a status whose holder nothing can see while it
///   lasts; `false`, the default, otherwise.
```

Any other exhaustive `StatusDef { .. }` literal in the workspace (`grep -rn "StatusDef {" crates examples`) that lists every field adds `unseen: false`; those using `..StatusDef::new(..)` need nothing. Run the test and see it pass.

- [ ] **Step 2: Write the failing stealth tests**

In the tests module of `crates/rl-bevy/src/stealth.rs`:

```rust
    /// How many blows the watcher has swung, counted as they are written,
    /// since its blow is `flat(0)` and health cannot show one.
    #[derive(Resource, Default)]
    struct Swings(usize);

    fn count_swings(mut struck: MessageReader<crate::combat::Struck>, mut swings: ResMut<Swings>) {
        swings.0 += struck.read().count();
    }

    /// An unseen player is not noticed, however keen the watcher and
    /// however close: a watcher that notices anything in reach every turn
    /// stands one step away and never learns it is there.
    #[test]
    fn an_unseen_subject_is_not_noticed_even_adjacent() {
        let mut field = Field::new(keen(), 1, 10, true, None);
        field.app.init_resource::<Swings>().add_systems(PostUpdate, count_swings);
        let player = field.player;
        field.app.world_mut().entity_mut(player).insert(Unseen);
        for _ in 0..4 {
            field.wait();
        }
        assert!(!field.aware().knows(player), "adjacent, keen, and none the wiser");
        assert_eq!(field.app.world().resource::<Swings>().0, 0, "and never swung at");
    }

    /// A watcher alert to the player loses it the moment it is unseen and
    /// searches where it last saw it, then forgets.
    #[test]
    fn a_watcher_that_knew_loses_the_unseen_and_forgets_after_its_memory() {
        let mut field = Field::new(keen(), 4, 10, true, None);
        field.wait();
        assert!(field.aware().knows(field.player), "noticed first");
        let player = field.player;
        field.app.world_mut().entity_mut(player).insert(Unseen);
        for _ in 0..4 {
            field.wait();
        }
        assert!(!field.aware().knows(player), "a memory of three turns, and it forgot");
    }

    /// Without awareness, a mind that sees on sight still does not see the
    /// unseen.
    #[test]
    fn a_mind_that_sees_on_sight_does_not_see_the_unseen() {
        let mut field = Field::new(blind(), 1, 10, true, None);
        field.app.init_resource::<Swings>().add_systems(PostUpdate, count_swings);
        let (watcher, player) = (field.watcher, field.player);
        field.app.world_mut().entity_mut(watcher).remove::<(Notice, Aware)>();
        field.wait();
        assert!(field.app.world().resource::<Swings>().0 > 0, "seen, adjacent, it swings: the test can fail");
        field.app.world_mut().resource_mut::<Swings>().0 = 0;
        field.app.world_mut().entity_mut(player).insert(Unseen);
        for _ in 0..3 {
            field.wait();
        }
        assert_eq!(field.app.world().resource::<Swings>().0, 0, "unseen, adjacent, and never swung at");
    }

    /// The vitals strip's question: nobody watches the unseen.
    #[test]
    fn nobody_is_watching_the_unseen() {
        let mut field = Field::new(keen(), 2, 10, true, None);
        field.app.init_resource::<Watched>().add_systems(PostUpdate, read_watched);
        field.wait();
        assert_eq!(field.app.world().resource::<Watched>().0, Some(true));
        let player = field.player;
        field.app.world_mut().entity_mut(player).insert(Unseen);
        field.wait();
        assert_eq!(field.app.world().resource::<Watched>().0, Some(false));
    }
```

In `a_mind_that_sees_on_sight_does_not_see_the_unseen` the plugin is on and the watcher's `Notice` and `Aware` are taken off, so it sees on sight, as `a_watcher_that_sees_on_sight_counts_...` sets one up; its first assertion is the control that shows the watcher does swing at a player it can see.

Then the revealing and marking tests, which need statuses, and statuses are a plugin that must be added before the field's first update. Split `Field::new` so a test can add plugins before anything runs: rename its body to

```rust
        /// The field, with `extra` run on the app after the field's own
        /// plugins are added and before anything is spawned or updated.
        fn build(notice: NoticeStats, gap: i32, reach: i32, plugin: bool, light: Option<u8>, extra: impl FnOnce(&mut App)) -> Field {
```

calling `extra(&mut app);` right after the `if light.is_some() { .. }` block, and keep `new` as

```rust
        fn new(notice: NoticeStats, gap: i32, reach: i32, plugin: bool, light: Option<u8>) -> Field {
            Field::build(notice, gap, reach, plugin, light, |_| {})
        }
```

Then add a helper to the test module:

```rust
    /// The field, with statuses on and a `cloaked` status registered as
    /// unseen, returned with its id.
    fn cloaking_field(notice: NoticeStats, gap: i32) -> (Field, rl_rules::StatusId) {
        let mut field = Field::build(notice, gap, 10, true, None, |app| {
            app.add_plugins(crate::status::StatusPlugin);
        });
        let statuses = rl_rules::Registry::from_defs(vec![rl_rules::StatusDef::new("cloaked").unseen()]).unwrap();
        let cloaked = statuses.expect("cloaked");
        field.app.world_mut().resource_mut::<crate::registries::Registries>().statuses = statuses;
        (field, cloaked)
    }

    fn cloak(field: &mut Field, status: rl_rules::StatusId) {
        let player = field.player;
        field.app.world_mut().write_message(crate::status::Afflict { target: player, status, turns: 5, by: None });
        field.wait();
    }
```

```rust
    /// Holding an unseen status is being unseen, and losing it is being
    /// seen again.
    #[test]
    fn an_unseen_status_marks_its_holder_while_it_lasts() {
        let (mut field, cloaked) = cloaking_field(blind(), 6);
        cloak(&mut field, cloaked);
        assert!(field.app.world().get::<Unseen>(field.player).is_some());
        for _ in 0..6 {
            field.wait();
        }
        assert!(field.app.world().get::<Unseen>(field.player).is_none(), "five turns, and it wore off");
    }

    /// A blow ends it in the pass it is struck, before the damage lands, and
    /// the one struck knows where from.
    #[test]
    fn striking_from_the_unseen_ends_it_and_wakes_the_one_struck() {
        let (mut field, cloaked) = cloaking_field(blind(), 1);
        cloak(&mut field, cloaked);
        let (player, watcher) = (field.player, field.watcher);
        let kind = field.app.world().resource::<crate::registries::Registries>().damage_kinds.expect("kinetic");
        field.app.world_mut().entity_mut(player).insert(MeleeAttack::new(kind, DiceRoll::flat(1)));
        field.app.world_mut().write_message(Intent::new(player, crate::combat::Attack(watcher)));
        field.app.update();
        assert!(field.app.world().get::<Unseen>(player).is_none(), "the blow ended it");
        assert!(field.aware().knows(player), "and the one struck knows where from");
    }

    /// Throwing and an ability aimed at someone else end it as a blow does;
    /// an ability on oneself does not.
    #[test]
    fn a_throw_and_an_ability_at_another_end_it_and_one_on_yourself_does_not() {
        use crate::ability::AbilityEvent;
        let (mut field, cloaked) = cloaking_field(blind(), 6);
        let (player, watcher) = (field.player, field.watcher);
        let ability = rl_rules::ability::AbilityId::from_raw(0);
        let at = field.at(player);
        cloak(&mut field, cloaked);
        field.app.world_mut().write_message(AbilityEvent::Used { user: player, ability, aim: at, targets: vec![player] });
        field.wait();
        assert!(field.app.world().get::<Unseen>(player).is_some(), "a stim in the arm is not an attack");
        field.app.world_mut().write_message(AbilityEvent::Used { user: player, ability, aim: at, targets: vec![watcher] });
        field.wait();
        assert!(field.app.world().get::<Unseen>(player).is_none(), "one aimed at another is");

        cloak(&mut field, cloaked);
        let thing = field.app.world_mut().spawn(crate::items::Item).id();
        field.app.world_mut().write_message(crate::items::ItemEvent::Thrown { actor: player, item: thing, at: Position(at), struck: None });
        field.wait();
        assert!(field.app.world().get::<Unseen>(player).is_none(), "and so is a throw, whatever it hit");
    }
```

The field's app has no `ItemsPlugin` or `AbilitiesPlugin`, so writing those messages needs them registered: `StealthPlugin` registers what it reads (Step 3), which is the point of the "a game without items has nothing to reveal on" rule.

Run: `cargo test -p rl-bevy unseen`
Expected: FAIL to compile, `Unseen` not found.

- [ ] **Step 3: Implement the unseen in stealth**

In `crates/rl-bevy/src/stealth.rs`, add the imports `use crate::status::{Afflicted, Cure};`, `use crate::registries::Registries;`, and, after `Stealth`:

```rust
/// Nothing sees this actor: it holds a status registered as `unseen`.
///
/// Kept by [`mark_unseen`] from the statuses, never inserted by hand in a
/// game, so it lasts exactly as long as the status and a continued run
/// gets it back from the statuses it saved. While it is on, no mind
/// perceives its holder at any distance and no observer notices it. That
/// departs on purpose from the rule that no stack of [`Stealth`] makes
/// somebody standing next to you invisible: quiet is for good, and this
/// lasts turns and ends the moment its holder strikes.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Unseen;

/// Puts [`Unseen`] on whoever holds an unseen status and takes it off
/// whoever no longer does.
///
/// In [`ResolveSet::Effects`](crate::plugin::ResolveSet::Effects) after the
/// statuses are applied, ticked and cured, so the pass a cloak goes on is
/// the pass its wearer vanishes, and the pass a blow ends it is the pass
/// it is seen again.
pub fn mark_unseen(mut commands: Commands, registries: Option<Res<Registries>>, actors: Query<(Entity, &Afflicted, Has<Unseen>), Changed<Afflicted>>) {
    let Some(registries) = registries else { return };
    for (actor, afflicted, marked) in &actors {
        let unseen = afflicted.0.iter().any(|s| registries.statuses.get(s.id).unseen);
        match (unseen, marked) {
            (true, false) => {
                commands.entity(actor).insert(Unseen);
            }
            (false, true) => {
                commands.entity(actor).remove::<Unseen>();
            }
            _ => {}
        }
    }
}

/// Ends every unseen status on whoever made an attack this pass: a blow or
/// a shot, landed or not, a throw, or an ability landed on anyone but
/// themselves.
///
/// Before the statuses resolve in the same pass, so the cure lands before
/// the damage does and the one struck wakes to an attacker it can see.
/// Being hurt is not here: a grenade in the dark does not light you up.
pub fn reveal_attackers(
    mut struck: MessageReader<crate::combat::Struck>,
    mut items: MessageReader<crate::items::ItemEvent>,
    mut abilities: MessageReader<crate::ability::AbilityEvent>,
    registries: Option<Res<Registries>>,
    unseen: Query<&Afflicted, With<Unseen>>,
    mut cure: MessageWriter<Cure>,
) {
    let mut attackers: Vec<Entity> = struck.read().map(|s| s.attacker).collect();
    attackers.extend(items.read().filter_map(|e| match e {
        crate::items::ItemEvent::Thrown { actor, .. } => Some(*actor),
        _ => None,
    }));
    attackers.extend(abilities.read().filter_map(|e| match e {
        crate::ability::AbilityEvent::Used { user, targets, .. } if targets.iter().any(|t| t != user) => Some(*user),
        _ => None,
    }));
    let Some(registries) = registries else { return };
    attackers.sort();
    attackers.dedup();
    for who in attackers {
        let Ok(afflicted) = unseen.get(who) else { continue };
        for status in afflicted.0.iter().filter(|s| registries.statuses.get(s.id).unseen) {
            cure.write(Cure { target: who, status: status.id });
        }
    }
}

/// Takes the unseen out of what the mind holding the turn perceives,
/// enemies, allies and others alike.
///
/// In [`PerceiveSet::Filter`](crate::plugin::PerceiveSet::Filter) before
/// [`filter_unnoticed`], so a mind that was alert to a subject that has
/// just vanished is offered the trail to where it last saw it, as for
/// anything else out of sight. Every mind, noticing or not: a monster that
/// sees on sight sees nothing here either.
pub fn filter_unseen(mut thinking: ResMut<Thinking>, unseen: Query<(), With<Unseen>>) {
    let Some(snapshot) = thinking.snapshot_mut() else { return };
    snapshot.enemies.retain(|e| !unseen.contains(e.id));
    snapshot.allies.retain(|e| !unseen.contains(e.id));
    snapshot.others.retain(|e| !unseen.contains(e.id));
}
```

`Watchers` gains a field `unseen: Query<'w, 's, (), With<Unseen>>`, and `judge` opens with:

```rust
        if watcher == subject || self.unseen.contains(subject) {
            return false;
        }
```

`update_awareness`: add `Has<Unseen>` to `Subject` and `Hiding` as their last element, destructure it as `hidden`, and write `let in_view = !hidden && on_this_map && ...`.

`StealthPlugin::build`:

```rust
        use crate::plugin::{PerceiveSet, Reads, ResolveSet};
        app.add_message::<Noticed>()
            .add_message::<DamageDealt>()
            .add_message::<crate::accuracy::Missed>()
            // What ending the unseen reads and writes, registered here so a
            // game with stealth and no items, abilities or statuses has
            // nothing to reveal on rather than a panic.
            .add_message::<Cure>()
            .reads::<crate::combat::Struck>()
            .reads::<crate::items::ItemEvent>()
            .reads::<crate::ability::AbilityEvent>()
            .add_stream::<StealthRng>("StealthPlugin")
            .add_systems(Turn, update_awareness.in_set(DecideSet::Notice))
            .add_systems(Turn, (filter_unseen, filter_unnoticed).chain().in_set(PerceiveSet::Filter))
            .add_systems(Turn, wake_on_damage.in_set(TurnSet::React))
            .add_systems(Turn, reveal_attackers.in_set(ResolveSet::Effects).before(crate::status::resolve_afflictions))
            .add_systems(Turn, mark_unseen.in_set(ResolveSet::Effects).after(crate::status::tick_statuses));
```

Keep the plugin's existing `use crate::plugin::{DecideSet, Turn, TurnSet};` and merge the imports. If `reads` is not the name of the `Reads` trait's method, read `crates/rl-bevy/src/plugin.rs` near line 610 and use its name.

Module doc: after the paragraph that ends "and nothing happened".", add:

```rust
//! A status registered as `unseen` hides its holder from everything, adjacency
//! included, until it runs out or its holder attacks: see [`Unseen`]. Noise
//! is still heard, since hearing is not sight.
```

Export `Unseen` from `lib.rs`'s `pub use stealth::{...}` and the prelude.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p rl-bevy stealth` then `cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Docs**

`docs/design/stealth.md`: add a section after "8. The UI payoff" titled `## 8b. The unseen`, with:

```markdown
A status registered as `unseen` hides its holder from everything: no mind perceives it and no observer notices it, at any distance, adjacency included.
This departs on purpose from the rule in section 2 that no stack of gear makes somebody standing next to you invisible.
That rule is about `Stealth`'s quiet, which is for good; the unseen is a status that lasts turns and ends the moment its holder attacks.
An observer that was alert loses the unseen as it loses anything out of sight, and searches where it last saw it for its memory.
Noise is still heard, since hearing is not sight.
A blow or a shot, landed or not, a throw, or an ability landed on anyone but its user ends every unseen status its maker holds, in the same pass and before the damage lands, so the one struck wakes to an attacker it can see; being hurt does not end it.
It is part of stealth rather than a plugin of its own because it answers stealth's question, who can see whom, and two plugins would make a game order their filters by hand.
Radar sees nothing either; whether it should is an open question, and so is what a mind that walks into an unseen actor learns.
```

In section 9, amend the first bullet: "Two-way stealth: monsters hiding from the player." add after its last sentence: "The same is true of an unseen monster: the minds already cannot see one, and the player's screen still can."

Re-read `docs/guide/src/systems/stealth.md` and `statuses.md`: in `stealth.md` `The model` add the four systems and the marker, and in `The line` add "The engine decides that the unseen is seen by nothing and that attacking ends it; the game decides which status is unseen, for how long, and what grants it." In `statuses.md` add `unseen` to the fields of a status. Check, bless and style-check every page named.

`README.md`, the **Stealth and awareness** bullet: append ", and statuses that make their holder unseen until it strikes".

`CHANGELOG.md`:

```markdown
- A status can make its holder unseen: `StatusDef::unseen`, `unseen: true` in a status file. With `StealthPlugin`, no mind perceives an unseen actor and no observer notices it, adjacency included; an observer that knew searches where it last saw it; noise is still heard; and a blow or shot, a throw, or an ability landed on anyone else ends every unseen status its maker holds, before the damage lands. `Unseen` marks the holder. A `StatusDef { .. }` literal that names every field adds `unseen: false`.
```

- [ ] **Step 6: Commit**

```bash
git add crates docs README.md CHANGELOG.md
git commit -m "a status can make its holder unseen until it strikes"
```

---

## Task 9: Foundry's nanite and cloak plates

**Files:**
- Modify: `examples/foundry/src/content.rs:166-170` (the `cloaked` status)
- Modify: `examples/foundry/src/gear.rs` (module doc, `ItemDef`, `PulseDef`, `EnchantDef`, `validate_def`, `Armory`, `Armory::load`, `spawn_item`, `spawn_item_at`, `ItemMaker for Armory`, tests)
- Modify: `examples/foundry/src/save.rs:140-186` (`ItemSave`)
- Modify: `examples/foundry/src/run.rs` (the faded commando), `examples/foundry/src/plugin.rs` (register it)
- Modify: `examples/foundry/assets/items.ron`, `examples/foundry/assets/item_spawns.ron`
- Create: `examples/foundry/assets/levels.ron`
- Test: `examples/foundry/src/gear.rs` tests, `examples/foundry/src/save.rs` tests, `examples/foundry/src/loot.rs` tests
- Docs: `docs/OVERVIEW.md` (Foundry's section), `examples/foundry/DESIGN.md` if it lists gear, `CHANGELOG.md`

**Interfaces:**
- Consumes: `Pulse::every` (Task 3), `Attuned` (Task 4), `Inflict::per_level` and `Landing::level` (Task 5), `Provenance` (Task 6), `LevelTable`, `load_levels` (Task 7), `StatusDef::unseen`, `Unseen` (Task 8).
- Produces: `spawn_item_at(commands, armory, id, level: i32, registries) -> Entity`; `spawn_item` is `spawn_item_at` at level 0; `Armory::levels: LevelTable`; `ItemSave::level: i32`.

- [ ] **Step 1: The status**

In `examples/foundry/src/content.rs`, add to the statuses:

```rust
        // What a cloak plate's charge puts on its wearer: nothing sees them
        // until it runs out or they strike.
        StatusDef { badge: Some('%'), ..StatusDef::new("cloaked").unseen() },
```

Update the comment above `let statuses` to mention it, and the list in `examples/foundry/assets/abilities.ron`'s header comment ("the statuses ...") to include `"cloaked"`.

- [ ] **Step 2: The item file's three fields, test first**

Write the failing loader tests in `examples/foundry/src/gear.rs` tests:

```rust
    #[test]
    fn the_plates_load_with_their_clock_their_charge_and_their_level() {
        let r = crate::content::registries();
        let armory = crate::testing::armory(&r);
        let nanite = armory.defs.get(armory.defs.expect("nanite plate"));
        assert_eq!(nanite.pulse.map(|p| (p.every, p.per_level, p.fastest)), Some((1000, -100, 100)));
        assert_eq!(nanite.enchant.map(|e| e.most), Some(9));
        let cloak = armory.defs.get(armory.defs.expect("cloak plate"));
        assert!(cloak.attuned);
        assert_eq!(cloak.consumable.and_then(|c| c.recharge), Some(4000));
    }

    #[test]
    fn a_pulse_or_an_attunement_or_an_enchant_on_a_thing_never_worn_fails_to_validate() {
        let r = crate::content::registries();
        let armory = crate::testing::armory(&r);
        let mut d = blank_def("loose pulse");
        d.pulse = Some(PulseDef { every: 800, per_level: 0, fastest: 800 });
        assert!(validate_def(&d, &armory.defs).is_err(), "a pulse on nothing worn");
        let mut d = blank_def("loose attunement");
        d.attuned = true;
        assert!(validate_def(&d, &armory.defs).is_err(), "attuned and never worn");
        let mut d = blank_def("loose enchant");
        d.enchant = Some(EnchantDef { most: 2 });
        assert!(validate_def(&d, &armory.defs).is_err(), "an enchant on nothing worn");
    }
```

Add to `blank_def`: `pulse: None, attuned: false, enchant: None,`. Change `every_item_loads_...`'s count to 24 and its message to "sixteen things to carry, a slug, a keycard, a stim, a medkit and four grenades".

Run: `cargo test -p foundry plates` and see it fail to compile.

Add to `ItemDef`, after `dark_sight`:

```rust
    /// A clock that comes round while it is worn, for a thing whose `pulse`
    /// trigger does something on it.
    #[serde(default)]
    pub pulse: Option<PulseDef>,
    /// True for a thing whose charges come back only while it is worn, and
    /// which is empty each time it is put on.
    #[serde(default)]
    pub attuned: bool,
    /// How far a thing found about the decks may be enchanted; absent, it
    /// is always plain.
    #[serde(default)]
    pub enchant: Option<EnchantDef>,
```

and the two definitions after `ThrowDef`:

```rust
/// A worn thing's clock as `items.ron` writes it: hundredths of a step per
/// pulse at `+0`, what each level adds, and the shortest period any level
/// reaches.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct PulseDef {
    /// Hundredths per pulse at `+0`.
    pub every: u32,
    /// Hundredths added per level; negative for a clock that quickens.
    pub per_level: i32,
    /// The shortest period any level reaches.
    pub fastest: u32,
}

impl PulseDef {
    /// The period at `level`.
    pub fn at(&self, level: i32) -> u32 {
        (self.every as i32 + self.per_level * level.max(0)).max(self.fastest as i32).max(1) as u32
    }
}

/// How far a thing may be enchanted where it is found.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct EnchantDef {
    /// The most any one of it is found at.
    pub most: i32,
}
```

`validate_def` gains, before `Ok(())`:

```rust
    if d.slot.is_none() && (d.pulse.is_some() || d.attuned || d.enchant.is_some()) {
        return Err("a pulse, an attunement or an enchant on a thing that is never worn does nothing".into());
    }
    if d.pulse.is_some() != d.triggers.iter().any(|t| t.on == "pulse") {
        return Err("a pulse needs a pulse trigger to do something, and a pulse trigger needs a pulse to come round".into());
    }
    if d.attuned && d.consumable.and_then(|c| c.recharge).is_none() {
        return Err("attuned, and nothing that refills".into());
    }
    if d.enchant.is_some_and(|e| e.most < 1) {
        return Err("an enchant that reaches no level".into());
    }
```

- [ ] **Step 3: The two plates and where they lie**

In `examples/foundry/assets/items.ron`, add to the schema comment after `dark_sight`:

```ron
//   pulse:     optional; (every:, per_level:, fastest:) a clock that comes round while it is
//              worn, every `every` hundredths of a step at +0, `per_level` added per level,
//              never shorter than `fastest`; what it does is its "pulse" trigger, and a
//              thing with one must have the other
//   attuned:   optional; true for a worn thing whose charges come back only while it is
//              worn and which is empty each time it is put on
//   enchant:   optional; (most:) a worn thing found about the decks rolls a level from
//              levels.ron at the deck it is found on, at most `most`; absent, always plain
```

and change the `on is` list in `triggers:` to read `on is "use" (used from the pack, or while worn for a thing that is worn; lands on the user where they stand) | "land" ... | "pulse" (a worn thing's clock came round; lands on the wearer)`. After the armored greaves:

```ron
    // Knits its wearer's wounds, a point every ten turns while it is
    // worn, and a turn quicker for every level, to a point every turn at
    // +9. The clock starts when it goes on, so it is a plate to live in,
    // not to swap to.
    (name: "nanite plate", glyph: '[', color: (0.55, 0.9, 0.7), slot: "torso", tags: ["armor"], armor: 1,
     pulse: (every: 1000, per_level: -100, fastest: 100), enchant: (most: 9),
     triggers: [(on: "pulse", effects: [(kind: "Mend", args: (kind: "care", roll: "1"))])]),
    // Used while worn, nothing sees the commando for ten turns, two more
    // for every level, or until they strike. Its one charge comes back
    // over forty turns worn, and it is empty each time it goes on, so it
    // cannot be carried charged and swapped to.
    (name: "cloak plate", glyph: '[', color: (0.5, 0.55, 0.8), slot: "torso", tags: ["armor"], armor: 1,
     attuned: true, consumable: (charges: 1, when_empty: Kept, recharge: 4000), enchant: (most: 9),
     triggers: [(on: "use", effects: [(kind: "Inflict", args: (status: "cloaked", turns: 10, per_level: 2))])]),
```

In `examples/foundry/assets/item_spawns.ron`, after the armored greaves:

```ron
    // The two plates with a use of their own, rarer than plate that is
    // only plate: a cloak from the first deck, a mending plate from the
    // third.
    (item: "nanite plate",       bands: (3, 10), weight: 1),
    (item: "cloak plate",        bands: (1, 10), weight: 1),
```

Create `examples/foundry/assets/levels.ron`:

```ron
// How good a thing is where it is found: the enchant level a worn thing
// that names `enchant` in items.ron rolls at the deck it is found on, or
// the deck a crate or a slot draws at, capped at the thing's own `most`.
//
// Every field:
//   bands:  (first, last), the decks the row covers, both included; no two
//           rows share a deck, and a deck past the last row reads the last
//   levels: [(level, weight)], each level nought or more and each weight
//           more than nought; only the ratio of the weights matters, and a
//           row of one level is that level every time
[
    // An uncommon +1 from the first deck, one find in ten.
    (bands: (1, 2),   levels: [(0, 9), (1, 1)]),
    (bands: (3, 4),   levels: [(0, 6), (1, 3), (2, 1)]),
    (bands: (5, 6),   levels: [(0, 3), (1, 4), (2, 2), (3, 1)]),
    (bands: (7, 8),   levels: [(0, 1), (1, 3), (2, 3), (3, 2), (4, 1)]),
    (bands: (9, 9),   levels: [(1, 2), (2, 3), (3, 3), (4, 2)]),
    // +5 only here: the last deck, or a locker asking two decks down
    // from deck eight.
    (bands: (10, 10), levels: [(2, 2), (3, 3), (4, 3), (5, 2)]),
]
```

- [ ] **Step 4: Spawn at a level, and roll it where found**

In `examples/foundry/src/gear.rs`: add `const LEVELS_RON: &str = include_str!("../assets/levels.ron");`, a field on `Armory`:

```rust
    /// How good a found thing is at each deck, for the things that name an
    /// `enchant`: `levels.ron`.
    pub levels: LevelTable,
```

loaded in `Armory::load` with `let levels = rl_engine::rl_rules::loot::load_levels(LEVELS_RON).unwrap_or_else(|e| panic!("assets/levels.ron: {e}"));` and put in the returned struct.

Replace `spawn_item`'s signature and add the leveled one:

```rust
/// Spawns `id` plain, at `+0`: what a cheat, a save of an old run and a
/// test make.
pub fn spawn_item(commands: &mut Commands, armory: &Armory, id: Id<ItemDef>, registries: &Registries) -> Entity {
    spawn_item_at(commands, armory, id, 0, registries)
}
```

Rename the existing body to `spawn_item_at(commands: &mut Commands, armory: &Armory, id: Id<ItemDef>, level: i32, registries: &Registries) -> Entity`, update its doc with "at `level`, which names it `cloak plate +2`, lands its effects at that level and writes its clock with the level applied; a thing that names no `enchant` is plain whatever `level` says", and inside it:

```rust
    let d = armory.defs.get(id);
    let level = if d.enchant.is_some() { level.max(0) } else { 0 };
    let enchanted = Enchanted { level, affixes: Vec::new() };
    let name = enchanted.display_name(&d.name, &Registry::<AffixDef>::default());
    let mut e = commands.spawn((Item, ItemKind(id), Name::new(name), Glyph::new(d.glyph, Color::srgb(d.color.0, d.color.1, d.color.2)).on_layer(2)));
    if d.enchant.is_some() {
        e.insert(Enchant(enchanted));
    }
```

and, after the `dark_sight` insert:

```rust
    if let Some(pulse) = d.pulse {
        e.insert(Pulse::every(pulse.at(level)));
    }
    if d.attuned {
        e.insert(Attuned);
    }
```

Import `Enchanted`, `AffixDef`, `LevelTable` from `rl_engine::rl_rules` and `Pulse`, `Attuned`, `Enchant`, `Provenance` from the engine's prelude or `rl_engine::rl_bevy` as the file's imports already do.

The maker:

```rust
    fn make(&self, commands: &mut Commands, registries: &Registries, def: Id<ItemDef>, count: u32, from: Provenance, rng: &mut rand::rngs::StdRng) -> Vec<Entity> {
        let d = self.defs.get(def);
        if d.stack {
            return spawn_items(commands, self, def, count, registries);
        }
        // Each its own roll: two plates from one crate are two finds.
        (0..count)
            .map(|_| {
                let level = d.enchant.map_or(0, |e| self.levels.roll(from.band, rng).min(e.most));
                spawn_item_at(commands, self, def, level, registries)
            })
            .collect()
    }
```

Update the `ItemMaker for Armory` doc: "... and asks the armory to make each thing, rolling a level from `levels.ron` at the band it is found at for a thing that names an `enchant`."

- [ ] **Step 5: The plates' own tests**

Add to `examples/foundry/src/gear.rs` tests:

```rust
    /// The nanite plate knits a point back every ten turns worn, counted
    /// from when it went on.
    #[test]
    fn the_nanite_plate_mends_a_point_every_ten_turns_worn() {
        let (mut app, player) = alone_and_wounded(11, 10);
        let before = health(&app, player);
        crate::testing::equip_new(&mut app, player, "nanite plate");
        crate::testing::pass_turns(&mut app, 8);
        assert_eq!(health(&app, player), before, "not yet");
        crate::testing::pass_turns(&mut app, 2);
        assert_eq!(health(&app, player), before + 1, "a point in ten turns");
    }

    /// Every `DamageDealt` on anyone, recorded as it is written: a headless
    /// app rotates its message buffers on wall time, so draining the queue
    /// afterwards can find it already empty and prove nothing.
    #[derive(Resource, Default)]
    struct Dealt(Vec<(Entity, i32)>);

    fn record_dealt(mut dealt: MessageReader<DamageDealt>, mut out: ResMut<Dealt>) {
        out.0.extend(dealt.read().map(|d| (d.target, d.dealt)));
    }

    /// Worn at full health it mends nothing and says nothing: a whole
    /// commando's log is not a list of mends.
    #[test]
    fn the_nanite_plate_on_a_whole_commando_says_nothing() {
        let (mut app, player) = alone_and_wounded(12, 0);
        app.init_resource::<Dealt>().add_systems(PostUpdate, record_dealt);
        crate::testing::equip_new(&mut app, player, "nanite plate");
        crate::testing::pass_turns(&mut app, 21);
        let mine: Vec<i32> = app.world().resource::<Dealt>().0.iter().filter(|(t, _)| *t == player).map(|(_, d)| *d).collect();
        assert_eq!(mine.len(), 2, "two pulses came round in twenty-one turns: {mine:?}");
        assert!(mine.iter().all(|d| *d == 0), "and each mended nothing, so the narrator says nothing: {mine:?}");
    }

    /// A `+2` nanite plate comes round every eight turns, a `+9` one every
    /// turn, and nothing past `+9` any faster.
    #[test]
    fn a_nanite_plate_quickens_with_its_level_to_every_turn_at_plus_nine() {
        let r = crate::content::registries();
        let armory = crate::testing::armory(&r);
        let nanite = armory.defs.get(armory.defs.expect("nanite plate")).pulse.unwrap();
        assert_eq!((nanite.at(0), nanite.at(2), nanite.at(9), nanite.at(12)), (1000, 800, 100, 100));
    }

    /// Every attacker's swing, recorded as it is written: a miss spends no
    /// health, and a test that read health would take a miss for a droid
    /// that never swung.
    #[derive(Resource, Default)]
    struct Swung(Vec<Entity>);

    fn record_swings(mut struck: MessageReader<Struck>, mut swung: ResMut<Swung>) {
        swung.0.extend(struck.read().map(|s| s.attacker));
    }

    /// The cloak plate is empty when it goes on and charged forty turns
    /// later; used then, a droid alert to the commando one step away never
    /// swings at them while it lasts.
    #[test]
    fn the_cloak_plate_hides_the_commando_from_an_adjacent_droid_once_it_has_charged() {
        let (mut app, player) = alone_and_wounded(15, 0);
        let plate = crate::testing::equip_new(&mut app, player, "cloak plate");
        assert_eq!(app.world().get::<Consumable>(plate).map(|c| c.left), Some(0), "empty when it went on");
        crate::testing::pass_turns(&mut app, 40);
        assert_eq!(app.world().get::<Consumable>(plate).map(|c| c.left), Some(1), "charged after forty turns worn");
        // Only now the droid, so forty turns of waiting are not forty turns
        // of being hit.
        let (droid, _) = crate::testing::droid_facing_player(&mut app, "line droid", 1);
        crate::testing::alert(&mut app, droid, player);
        app.init_resource::<Swung>().add_systems(PostUpdate, record_swings);
        app.world_mut().write_message(Intent::new(player, UseItem(plate)));
        crate::testing::settle(&mut app);
        assert!(app.world().get::<Unseen>(player).is_some(), "cloaked");
        crate::testing::pass_turns(&mut app, 3);
        assert!(!app.world().resource::<Swung>().0.contains(&droid), "never swung at while cloaked");
    }

    /// A `+2` cloak plate hides the commando for fourteen turns.
    #[test]
    fn a_cloak_plates_level_adds_a_turn_each() {
        let (mut app, player) = alone_and_wounded(13, 0);
        let registries = app.world().resource::<Registries>().clone();
        let armory = crate::testing::armory_of(&app);
        let plate = {
            let mut queue = CommandQueue::default();
            let mut commands = Commands::new(&mut queue, app.world_mut());
            let plate = spawn_item_at(&mut commands, &armory, armory.defs.expect("cloak plate"), 2, &registries);
            queue.apply(app.world_mut());
            plate
        };
        assert_eq!(app.world().get::<Name>(plate).map(|n| n.as_str().to_string()), Some("cloak plate +2".to_string()));
        app.world_mut().get_mut::<Inventory>(player).unwrap().items.push(plate);
        app.world_mut().write_message(Intent::new(player, Equip(plate)));
        app.update();
        app.world_mut().get_mut::<Consumable>(plate).unwrap().left = 1;
        app.world_mut().write_message(Intent::new(player, UseItem(plate)));
        crate::testing::settle(&mut app);
        let cloaked = app.world().resource::<Registries>().statuses.expect("cloaked");
        let turns = app.world().get::<Afflicted>(player).and_then(|a| a.0.iter().find(|s| s.id == cloaked).map(|s| s.turns));
        assert_eq!(turns, Some(14), "ten and two for each of two levels");
    }
```

`run_until_struck` and its result type live in `examples/foundry/src/testing/droids.rs:93`; read it and use its real field names in the last assertion of the cloak test (the assertion's meaning: the droid did not strike the commando within the window). If the helper cannot express "not struck within N turns", replace those two lines with: pass 3 turns and assert the commando's `Health` is unchanged, since `Invulnerable` was put on only to keep the test alive and a blow would still write a `DamageDealt` of nought: count `DamageDealt` messages with the droid as attacker instead of reading health.

And in `examples/foundry/src/loot.rs` tests, the level ramp, property over seeds:

```rust
    /// What the decks hand out gets better the deeper it is, over a
    /// thousand draws a deck: deck one is plain but for the odd `+1`, deck
    /// nine is never plain, and `+5` waits for the last band.
    #[test]
    fn enchant_levels_climb_from_a_rare_plus_one_on_deck_one_to_plus_five_on_the_last() {
        let registries = crate::content::registries();
        let armory = crate::testing::armory(&registries);
        let mut rng = rand::rngs::StdRng::seed_from_u64(7);
        let mut draws = |band: i32| (0..1000).map(|_| armory.levels.roll(band, &mut rng)).collect::<Vec<i32>>();
        let first = draws(1);
        let ones = first.iter().filter(|l| **l == 1).count();
        assert!(first.iter().all(|l| (0..=1).contains(l)), "deck one is plain or +1");
        assert!((50..=160).contains(&ones), "and +1 is an uncommon surprise, about one in ten: {ones} of 1000");
        assert!(draws(9).iter().all(|l| (1..=4).contains(l)), "deck nine is never plain, and +5 waits for the last");
        let last = draws(10);
        assert!(last.contains(&5) && last.iter().all(|l| (2..=5).contains(l)), "the last deck reaches +5");
        assert!(draws(40).iter().all(|l| (2..=5).contains(l)), "a crate past the last deck reads the last row");
    }
```

Run: `cargo test -p foundry`
Expected: PASS.

- [ ] **Step 6: Save the level**

In `examples/foundry/src/save.rs`, `ItemSave` gains:

```rust
    /// Its enchant level; nought for a plain thing and for a save written
    /// before levels were.
    #[serde(default)]
    pub level: i32,
```

`capture` writes `level: world.get::<Enchant>(entity).map_or(0, |e| e.level)`, and `restore` spawns through `crate::gear::spawn_item_at(commands, armory, id, saved.level, world.resource::<Registries>())`. Update the `ItemSave` doc: "what it was made from, the level it was found at, and how hot it runs if it runs hot at all". No `VERSION` bump: an old save reads as level nought.

Write the test beside `a_run_saved_on_deck_three_comes_back_as_it_was`, using the module's own `player` helper:

```rust
    /// A cloak plate found at `+2`, worn and half charged, comes back a
    /// `+2`, worn, and as far charged as it was: a continued run is not a
    /// fresh putting-on.
    #[test]
    fn a_leveled_worn_thing_comes_back_at_its_level_and_its_charge() {
        let mut app = crate::testing::headless(RunSeed(4));
        crate::testing::settle(&mut app);
        let me = player(&mut app);
        let registries = app.world().resource::<Registries>().clone();
        let armory = crate::testing::armory_of(&app);
        let plate = {
            let mut queue = CommandQueue::default();
            let mut commands = Commands::new(&mut queue, app.world_mut());
            let plate = crate::gear::spawn_item_at(&mut commands, &armory, armory.defs.expect("cloak plate"), 2, &registries);
            queue.apply(app.world_mut());
            plate
        };
        app.world_mut().get_mut::<Inventory>(me).unwrap().items.push(plate);
        app.world_mut().write_message(Intent::new(me, Equip(plate)));
        app.update();
        crate::testing::pass_turns(&mut app, 20);
        let progress = |app: &App, item: Entity| app.world().get::<Consumable>(item).and_then(|c| c.recharge).map(|r| r.progress);
        let before = progress(&app, plate);
        assert!(before.is_some_and(|p| p > 0), "half charged: {before:?}");

        save_run(app.world_mut()).expect("the run saves");
        let text = app.world().resource::<Saves>().load(SLOT).unwrap().expect("a save");
        let mut continued = crate::testing::continued(&text);
        let me = player(&mut continued);
        let world = continued.world();
        let back = world
            .get::<Inventory>(me)
            .unwrap()
            .items
            .iter()
            .copied()
            .find(|i| world.get::<Name>(*i).is_some_and(|n| n.as_str() == "cloak plate +2"))
            .expect("a cloak plate +2 in the pack");
        assert!(world.get::<Equipped>(me).is_some_and(|w| w.slot_of(back).is_some()), "and worn");
        assert_eq!(progress(&continued, back), before, "charged as far as it was");
    }
```

Run `cargo test -p foundry a_leveled_worn_thing_comes_back`: it fails on the name (the plate comes back plain) until the two edits above are in, then passes.

- [ ] **Step 7: The commando fades while unseen**

In `examples/foundry/src/run.rs`, after `spawn_commando`:

```rust
/// How the commando is drawn: white, and faded while nothing can see them,
/// so the player sees the cloak working on the map and not only in a badge.
pub const COMMANDO: Color = Color::WHITE;
/// The commando while unseen.
pub const COMMANDO_UNSEEN: Color = Color::srgb(0.45, 0.5, 0.6);

/// Draws the commando faded while unseen and white otherwise.
///
/// In `TurnSet::React` rather than a drawing layer: the glyph's colour is
/// the run's state, set in the pass the cloak went on or came off, and the
/// map draws whatever it is.
pub fn fade_the_unseen(mut commando: Query<(&mut Glyph, Has<Unseen>), With<Commando>>) {
    for (mut glyph, unseen) in &mut commando {
        let fg = if unseen { COMMANDO_UNSEEN } else { COMMANDO };
        if glyph.fg != fg {
            glyph.fg = fg;
        }
    }
}
```

and `spawn_commando` uses `Glyph::new('@', COMMANDO)`. Register it in `examples/foundry/src/plugin.rs` beside the other `TurnSet::React` systems (line 74 onwards): `app.add_systems(Turn, crate::run::fade_the_unseen.in_set(TurnSet::React));`. Test it in `gear.rs`' tests, which have `alone_and_wounded`, through the real cloak so the pass that sets it is the one a player sees:

```rust
    /// The commando's `@` fades the pass the cloak goes on and is white
    /// again the pass it wears off.
    #[test]
    fn the_commando_is_drawn_faded_while_unseen() {
        let (mut app, player) = alone_and_wounded(14, 0);
        let fg = |app: &App| app.world().get::<Glyph>(player).map(|g| g.fg);
        assert_eq!(fg(&app), Some(crate::run::COMMANDO));
        let cloaked = app.world().resource::<Registries>().statuses.expect("cloaked");
        app.world_mut().write_message(Afflict { target: player, status: cloaked, turns: 2, by: None });
        crate::testing::pass_turns(&mut app, 1);
        assert_eq!(fg(&app), Some(crate::run::COMMANDO_UNSEEN), "faded while unseen");
        crate::testing::pass_turns(&mut app, 3);
        assert_eq!(fg(&app), Some(crate::run::COMMANDO), "and white once it wore off");
    }
```

- [ ] **Step 8: Docs and the whole run**

`docs/OVERVIEW.md`, Foundry's section: add a sentence: "The nanite plate mends its wearer on a worn clock and the cloak plate, used worn, makes them unseen; both are enchanted by the deck they are found on from `levels.ron`." If `examples/foundry/DESIGN.md` lists the gear, add the two plates there.

`CHANGELOG.md`:

```markdown
- Foundry has two torso plates with a use of their own: the cloak plate, found from deck one, used worn hides the commando for ten turns, or until they strike, and takes forty turns worn to charge; the nanite plate, found from deck three, mends a point every ten turns worn. Worn gear found about the decks rolls a level from `levels.ron`, an uncommon `+1` from deck one and up to `+5` on the last; each level takes a turn off the nanite plate's mend, to every turn at `+9`, and adds two turns to the cloak, and the save keeps it. The commando is drawn faded while unseen, and `cloaked` wears the `%` badge.
```

Run every check in Global Constraints. Then `cargo run -p foundry -- --prefabs` to see the coverage report still finds something for every slot.

- [ ] **Step 9: Commit**

```bash
git add examples docs CHANGELOG.md
git commit -m "foundry: a nanite plate that mends, a cloak plate that hides, and gear found better the deeper it lies"
```

---

## Task 10: In the running game

**Files:** whatever the play-through finds wrong.

This is the user's rule: the change is not done until it has been seen working the way a player would see it, and anything that looks off on screen gets fixed along the way.

- [ ] **Step 1: Launch Foundry**

Use the `run` skill to launch `cargo run -p foundry` and drive it. Start a new run.

- [ ] **Step 2: The nanite plate**

Open the cheat menu (`\`), search (`s`) for "nanite plate", take it, open the bag, and check the row: its armor, "goes on the torso", and "every 10 turns worn: mends 1 care" on one line each, wrapped cleanly at the panel width. Put it on. Take a hit from a deck-one enemy, then wait: the log says "You mend for 1." once each ten turns and says nothing once whole. Screenshot the bag and the log.

- [ ] **Step 3: The cloak plate**

Search for "cloak plate", take it, and check that the bag does not offer the use key while it is in the bag and says "charges only while worn". Put it on: the row says "ready in 40 turns", and the number falls as turns pass. When it is ready, stand in a droid's view and use it: the `%` badge shows on the vitals strip, the commando's `@` fades, the strip reads unwatched, and a droid that was hunting walks to where it last saw the commando and stops there. Walk up to a droid and strike it: the `@` is white again, the badge is gone, and the droid answers. Screenshot each state.

- [ ] **Step 4: Levels**

Take the cheat lift down to deck ten and open crates until an enchanted plate turns up: its name reads `cloak plate +N` or `nanite plate +N`, and its bag line says the level's number of turns. Save by quitting to the menu, continue, and check the plate is still `+N`, still worn, and the charge is where it was.

- [ ] **Step 5: Fix what looked off**

Anything misaligned, clipped, mis-toned or worded wrong on any screen seen above, related or not, is fixed now, with a test where one can be written, and committed on its own with a message saying what is now true.

- [ ] **Step 6: Final checks**

Run every check in Global Constraints once more on the branch head, then hand off with superpowers:finishing-a-development-branch.
