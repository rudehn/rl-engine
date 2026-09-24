# Effects as a subsystem, and items that say what they do

Status: design, agreed in conversation on 2026-09-23 against `main` at `b3ea6c6` plus the uncommitted Foundry content work.
Nothing here is built yet.

## 1. The problem

Items do things in two unrelated ways today.

- A thing that just happens to whoever used it carries `OnUse`, landed by `consumable::land_uses`.
- A thing that is aimed lends an ability with `Grants`, and that ability costs `Charge(1)`, which the engine spends off the item that lent it.

Foundry's four grenades are the second kind, so a grenade is written as an ability in `abilities.ron` whose cost destroys the item that lent it.
That is the mixing this design removes: an ability is something an actor knows how to do, and a grenade is not something the commando knows.

Underneath, effects are not a subsystem of their own.
The `Effect` trait, `Landing`, `EffectWorld`, `EffectKinds` and `AbilityRng` all live in `crates/rl-bevy/src/ability.rs`, so props and consumables reach into abilities to land anything, and `PropsPlugin` and `ConsumablesPlugin` each register the ability stream themselves.

Props already have the shape items want: a trigger names the moment that sets it off (`Entered`, `Destroyed`), how many times it may fire, and an effect list.

## 2. What was decided

These were settled in conversation and are not reopened here.

1. **Effects become a subsystem of their own**, which abilities, props and items each use as peers.
2. **Items carry triggers, not `Grants`.** An item never lends an ability. `Grants` stays for actors: innate abilities, and granted ones such as Foundry's stims upgrade.
3. **Triggers are one component and the moments are an open registry** (approach B below). Props and items share the shape.
4. **An item is aimed in one of two ways, each owned by an existing subsystem.** It is thrown, and its `land` trigger fires where it comes to rest; or it is fired as a weapon, and its `hit` trigger fires on whoever the attack struck. There is no third "aimed use" with a cursor of its own. A wand is a weapon that fires effects rather than damage.
5. **Firing a weapon stays combat's.** `Loadout`, `Struck`, the heat and ammunition a game hangs on it, `ShootAtRange`, the forecast and the narration are untouched.
6. **`Consumable` is a count of charges, not a marker**, with what happens at zero and an optional recharge.
7. **`on_equip` stays deferred**, for the reason `docs/design/items.md` gives: effects happen once and wearing is a standing state.
8. **Giving a shot a shape** (a cone for a scattergun, a beam for a lance) is a combat change and is out of scope; `hit` triggers will work with it unchanged once it exists.

### The approaches weighed

- **A. One component per moment** (`OnUse`, `OnLand`, `OnHit`), each landed by the plugin that owns the moment. Typed and plain, but the rule "find the list, build the footprint, land it, spend a charge" is written once per moment and again for props, and a game cannot add a moment of its own without writing another component and system.
- **B. One `Triggers` component and a registry of moments**, chosen. One shape for props and items; a game adds a moment with one line; spending lives in one place. The cost is one message hop, which the ordering in section 7 pins down.
- **C. Items as offers**, the shape props' `open` uses. It brings a verb, a time cost and refusal reasons, but only for a use: a landing or a hit is not an act anyone chooses, so every item file would be offers plus A-style fields.

## 3. The model in content

A game's item file is its own schema; the engine reads only the components the game builds from it.
This is how Foundry's and Corsair's files will read, and the shape `TriggerSpec` deserializes.

```ron
// Used only.
(name: "stim", stack: true,
 consumable: (charges: 1, when_empty: Destroyed),
 triggers: [(on: "use", effects: [(kind: "Mend", args: (kind: "care", roll: "2d4+5"))])]),

// Thrown only: comes down, bursts, and is gone.
(name: "frag grenade", stack: true, throw: (range: 6),
 consumable: (charges: 1, when_empty: Destroyed),
 triggers: [(on: "land", area: Burst(radius: 1), effects: [(kind: "Harm", args: (kind: "kinetic", roll: "3d6"))])]),

// One list, two deliveries: what is in the bottle, drunk or thrown.
(name: "healing draught", stack: true, throw: (range: 5),
 consumable: (charges: 1, when_empty: Destroyed),
 effects: [(kind: "Mend", args: (kind: "care", roll: "2d6"))],
 triggers: [(on: "use"), (on: "land", area: Burst(radius: 1))]),

// Two deliveries, two lists.
(name: "lamp oil", stack: true, throw: (range: 5),
 consumable: (charges: 1, when_empty: Destroyed),
 triggers: [
     (on: "use", effects: [(kind: "Inflict", args: (status: "nauseous", turns: 3))]),
     (on: "land", area: Burst(radius: 1), effects: [(kind: "Ignite", args: (turns: 4))]),
 ]),

// A wand: a weapon whose shot carries effects and spends a charge.
(name: "arc wand", slot: "main hand", ranged: (range: 7),
 consumable: (charges: 5, when_empty: Kept, recharge: 2000),
 triggers: [(on: "hit", effects: [(kind: "Harm", args: (kind: "electricity", roll: "2d6"))])]),

// A gun whose hits set the floor alight; its economy is still slugs.
(name: "incendiary pistol", slot: "main hand", ammo: "slug",
 ranged: (range: 6, roll: "1d6", kind: "kinetic"),
 triggers: [(on: "hit", effects: [(kind: "Ignite", args: (turns: 2))])]),
```

A trigger with no `effects` of its own lands the item's shared `effects`.
That is the whole of the potion model: what a thing contains is written once, and each trigger is a way of delivering it.

Props read the same way in the engine's own `props.ron` schema:

```ron
(name: "live cable", hidden: (spot: 35),
 triggers: [(on: "entered", fires: 3, effects: [(kind: "Harm", args: (kind: "electricity", roll: "1d4"))])]),
(name: "fuel drum", health: 5, blocks: true,
 triggers: [(on: "destroyed", area: Burst(radius: 1), effects: [(kind: "Ignite", args: (turns: 4))])]),
```

## 4. The effects subsystem

`crates/rl-bevy/src/effects.rs` becomes the whole subsystem, under a new `EffectsPlugin`.

**Moved out of `ability.rs`:** the `Effect` and `FromArgs` traits, `Landing`, `EffectWorld`, `EffectKinds` with `AddEffect`, and the stream.

- The stream is renamed `EffectRng` and keeps its derivation domain, `b"ability"`, so every seed rolls the dice it rolled before.
- `Landing::ability: Option<AbilityId>` becomes `source: Source`, one of `Ability(AbilityId)`, `Trigger { on: Entity, moment: MomentId }` or `Offer(Entity)`, so an effect can tell what landed it without assuming an ability.
- `AbilitiesPlugin`, `PropsPlugin` and `ConsumablesPlugin` depend on `EffectsPlugin` with `depends_on` rather than registering its pieces themselves.
- `add_engine_effects` stays, and needs `EffectsPlugin`.

**Moments** follow the `Verbs` precedent in `crates/rl-bevy/src/props.rs` exactly.

- `Moments` is an `Interner<Moment>` resource. The engine's own are interned first so their ids are constants: `Moments::USE`, `LAND`, `FIRE`, `HIT`, `ENTERED`, `DESTROYED`.
- A game adds its own with `app.add_moment("overheat")` while the app is built, and looks the id up with `Moments::get`.
- A content file names a moment as a string. It is resolved when the triggers are built, and an unknown one fails that build with its name, the same way an unregistered effect kind does.

**The authored form**, in `rl-rules` beside `EffectSpec`, so any game's file can read it:

```rust
pub struct TriggerSpec { pub on: String, pub area: Area, pub fires: Option<u32>, pub effects: Option<Vec<EffectSpec>> }
pub enum Area { Here, Burst { radius: i32 } }
```

`Area` defaults to `Here`.
`Burst` is the same wall-bounded footprint an ability's `Ball` covers, so a grenade and a fireball of one radius reach the same cells.

**The component:**

```rust
pub struct Trigger { pub on: MomentId, pub area: Area, pub fires: Option<u32>, pub effects: Arc<Effects> }
pub struct Triggers(pub Vec<Trigger>);
```

`Triggers::build(specs, shared, moments, kinds, names)` builds a list once per definition, sharing each effect list through `Arc` so every copy of an item or prop points at the same one.
A spec with no effects of its own takes `shared`; a spec with neither is an error, since a trigger that does nothing is a typo.
`fires` is per entity: the `Arc` is shared, the count is not, so springing one cable does not spend another.

**The message and the landing:**

```rust
pub struct Fired { pub on: Entity, pub moment: MomentId, pub by: Option<Entity>, pub at: Point }
```

A subsystem that owns a moment writes `Fired` and does nothing else.
`land_triggers`, in `EffectsPlugin`, reads each `Fired`, takes the entity's triggers for that moment in list order, skips any whose `fires` is spent, builds the footprint from `area` around `at`, and lands the effects with `by` as the user and every actor under the footprint as a target, the user included.
It then counts `fires` down.

## 5. `Consumable`

```rust
pub struct Consumable { pub left: u16, pub max: u16, pub when_empty: WhenEmpty, pub recharge: Option<Recharge> }
pub enum WhenEmpty { Destroyed, Kept }
pub struct Recharge { pub every: u32, pub progress: u32 }
```

**Which moments spend** is a property of the moment, kept by `ConsumablesPlugin`: the engine marks `use`, `land` and `fire`, and a game may mark its own.
`hit` does not spend, since one shot can strike and a flaming blade is not used up by landing a blow.

**Spending lives in one system, `spend_charges`**, in `ConsumablesPlugin`, ordered after `land_triggers`.
It reads the same `Fired` messages and spends one charge from any `Consumable` whose moment spends, whether or not it had triggers, so a plain wand firing an ordinary shot still loses a charge.
`EffectsPlugin` knows nothing about charges.

**The ladder:** `left` goes down by one; at zero, a stack of more than one loses one and the next unit starts at `max`; otherwise `Destroyed` despawns the thing wherever it is and `Kept` leaves it at zero.
A thrown consumable is the single unit the throw split off the stack, so it is despawned where it came down rather than left lying.

**An empty `Kept` thing does nothing it is spent by:** `Loadout` stops finding its attack, as Foundry's dry pistol works today; a use of it is refused with a reason and costs no turn.
It can still be thrown or dropped.

**Recharge** runs on the integer turn clock: while `left < max`, `progress` counts the time that passes, and each `every` returns one charge and carries the remainder.

**Merging a stack** keeps the receiving stack's `left`, so a stack with a partly spent unit in front absorbs fresh units without refilling or emptying it.

## 6. What leaves abilities, and what stays

**Leaves:**
- items as a source of `Known`: `refresh_known` reads the actor's own `Grants` only, and `Known::source_of` and the item source it tracks go;
- `redirect_item_uses`;
- `Charges`, replaced by `Consumable`;
- `Cost::Charge`, since abilities never spend items;
- `OnUse` and `land_uses`, replaced by a `use` trigger and `land_triggers`.

**Stays:** `Grants` on actors, `Cost::Item(tag, n)` for an ability that spends a tagged thing from the bag, and everything about targeting, gating and cooldowns.

## 7. Timing

`land_triggers` runs in `ResolveSet::Effects`, after every reporter in `ResolveSet::Travel` and `ResolveSet::Act` and before `ResolveSet::Damage`, with `spend_charges` chained after it in the same set.

| Moment | Reported by | Set | Lands |
|---|---|---|---|
| `use` | the items resolver, on an accepted `UseItem` | `ResolveSet::Act` | the same pass |
| `land` | throwing, when a throw comes to rest | `LandSet::Throw` | the same pass |
| `fire` | combat, when an attack is made with a worn item | `ResolveSet::Act` | the same pass |
| `hit` | combat, when that attack strikes, at the struck actor's cell | `ResolveSet::Act` and `LandSet::Shot` | the same pass |
| `entered` | props, on a `Stepped` onto the prop's cell, which `resolve_moves` writes in `ResolveSet::Travel` | `ResolveSet::Act` | the same pass |
| `destroyed` | props, on the prop's `DeathEvent` | `TurnSet::React` | the next pass |

Two timings move against today, both checked against `props.rs`, where both prop triggers are landed in `TurnSet::React`:

- **A trap lands one pass sooner.** Its damage is applied in the pass the step was taken, where today it waits for the next pass's `Damage`.
- **A `destroyed` trigger's effects land one pass later**, since a death is only known after `Damage`. Damage it deals is applied in the same pass as today; a status or a fire it starts begins one pass later.

Foundry's live cables make the first change reach its fingerprint, which is re-baselined with a CHANGELOG line if it moves.

## 8. Saving, screens, determinism and minds

**Saving:** only what changes in play.
`Consumable`'s `left` and `recharge.progress` are saved beside the pools and cooldowns `EngineSave` already keeps for every saved entity, and each trigger's remaining `fires` with them.
The effect lists are never saved; a game rebuilds them from its definitions when it spawns the entity.

**The pack screen:**
- `ItemRow::usable` is true for a thing with a `use` trigger and a charge to spend; an empty one reads unusable and says why.
- The detail lists each trigger through `Effects::describe`: `use: mends 2d4+5`, `thrown: 3d6 kinetic in a burst of 1`.
- A thing with more than one charge shows them as a tag, `3/5`; a stack shows its count as today.

**The gear panel** shows a worn wand's charges the same way.

**Narration** gains no lines: using, throwing and firing are already narrated, and what effects do is narrated by the damage and status lines they cause.

**Determinism:** every effect rolls from `EffectRng`; `Fired` is read in the order it was written, triggers in list order, footprint cells in a fixed order.

**Minds** lose one thing: a mind could drink a healing potion only by using the ability the potion lent, and that goes with item `Grants`.
Firing a wand is `ShootAtRange` finding it through `Loadout` and throwing a grenade is `ThrowAtRange`, as today.
A tactic to use a thing from the bag when hurt goes in `docs/TODO.md`; no game does it today.

## 9. Migration

**Foundry:**
- `stim` and `medkit` move from `on_use` and `uses` to a `use` trigger and a `consumable`.
- The four grenades leave `abilities.ron`, which is back to `stims` alone, and become throwable items with a `land` trigger in a burst: radius 1, and 2 for the ion grenade.
- `t` becomes "throw from the pack", which picks the thing and then aims, as Corsair's does; today it throws the first throwable thing carried, which would pick between a blade and a grenade by bag order.
- `items.ron` gains `triggers`, `effects`, `consumable` and a `throw` whose strike is optional, and loses `on_use`, `uses` and `grants`; its header lists the full option space.
- `plugin/ambiguity.rs` names `Fired` in place of `Triggered`.

**Corsair:** the rum's `on_use` becomes a `use` trigger with a `consumable`. Its abilities are innate and unchanged.

**Delve, Heist, the tutorial and the template** carry no item effects and change only where a renamed type reaches them.

**Breaking changes, each with its CHANGELOG line:** `Triggered` becomes `Fired`; `OnUse` becomes a `use` trigger; `Charges` becomes `Consumable`; `Cost::Charge` and item `Grants` are removed; `AbilityRng` becomes `EffectRng`; the prop schema's `trigger:` becomes `triggers:` with moment names as strings.

## 10. Documentation, in the same change

- `docs/design/effects.md`, new: the subsystem, moments, triggers, the landing system, why moments are a registry, and approaches A and C. Listed in `CLAUDE.md`'s layout and in `docs/README.md`.
- `docs/design/items.md`, rewritten: the carriers as they now divide, the potion model, `Consumable` with charges, and why `on_equip` stays deferred.
- `docs/guide/src/systems/effects.md`, new, for `EffectsPlugin`, since `scripts/check-systems.py` fails on a plugin with no page; `items.md`, `props.md` and `abilities.md` updated and re-blessed.
- `docs/OVERVIEW.md`: the plugin table gains `EffectsPlugin`, and the effects, items and props lines are rewritten.
- `README.md`: the "Items that do things" bullet.
- `CHANGELOG.md`: the breaking changes in section 9.
- `docs/TODO.md`: a tactic for a mind to use a thing from its bag, a shape for a shot, and `on_equip`.
- `docs/PLAN.md`: a progress-log entry.

## 11. Testing

Each test is written to fail before the code it covers.

- **Moments:** an unknown moment fails the build naming it; a game's own moment, reported by a system of the game's, lands like an engine one.
- **Landing:** `Here` lands on one cell and `Burst` on every actor within the radius, the user included; walls bound a burst; `fires` runs out per entity and not per definition; a trigger with no effects takes the shared list; lists land in order.
- **Charges:** a stack is spent before the thing is destroyed; `Destroyed` against `Kept` at zero; recharge returns a charge per `every` on the clock and carries the remainder; an empty wand has no attack; a use of an empty thing is refused and costs no turn; merging keeps the receiving stack's `left`.
- **Moments in play:** a thrown consumable is gone where it came down and a thrown blade is not; firing a wand spends a charge on a miss; `hit` lands on the struck actor where it stands; a stim's mend is applied in the pass that spent the turn.
- **Props:** the existing trap and destroyed tests carry over with the timing in section 7 asserted; a burst on `destroyed` sets alight every cell in its radius.
- **Games:** Foundry's grenade tests become real throws through the throw action; the stim and medkit tests keep their numbers; Corsair's rum test carries over; the ambiguity test passes with `Fired`.
- **Saving:** a wand with two charges left and a part-filled recharge, and a cable with one fire left, come back as they were.

## 12. Out of scope

- A shape for a shot: cones and beams on `RangedAttack`.
- `on_equip`, and holding a status while something is worn.
- An aimed use with a cursor of its own.
- A mind using a thing from its bag.
- Refusal reasons on a use beyond "it is empty"; approach C's `needs` can join the `use` trigger later.
