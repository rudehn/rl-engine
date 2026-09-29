# One place for what a level does, and grenades that are heard

Status: design, agreed in conversation on 2026-09-27 against `main` at `ef9ec96`.
Nothing here is built yet.

## 1. What this is for

The worn-gear work left an item's enchantment written in three places and three shapes: a pulse's `(every, per_level, fastest)` at the top of the item, `per_level` inside an `Inflict`'s arguments, and `enchant: (most: N)` beside them, with `attuned` and `pulse` sitting at the top level of an item that has nothing else worn-only there.
A developer reading an item cannot tell at a glance what a `+2` does to it.
Only the two new plates take a level at all, so every other weapon and plate found on deck ten is the same as on deck one.
And a grenade bursting in a corridor makes no sound.

Done when:

- an item's `enchant` block is the one place that says it takes levels and everything a level changes, and no trigger or effect carries level rules;
- every weapon and piece of armor in Foundry is enchantable, a level adding to its armor or its damage by default;
- a pulse's period is written on its pulse trigger, and a charge that refills only while worn is written on its charges, with nothing worn-only left at an item's top level;
- a thrown grenade makes a noise where it lands, loud for a blast and quiet for smoke;
- Foundry plays with all of it, checked in the running game.

## 2. What was decided

These were settled in conversation and are not reopened here.

1. **Base numbers stay where they are; everything a level changes lives in `enchant`.**
2. **The `enchant` block's keys:**

   | Key | Meaning | Default |
   | --- | --- | --- |
   | `max` | the highest level it is found at; the block's presence makes it enchantable | required |
   | `armor` | armor added per level | +1 when the item has armor |
   | `damage` | added per level to the item's own attack roll: its blow, its shot, its throw's strike | +1 when the item attacks |
   | `dark_sight` | tiles of dark sight added per level | 0; never inferred |
   | `pulse` | hundredths added per level to its pulse trigger's period, never below one turn (100) | 0 |
   | `turns` | turns added per level to every status its effects inflict | 0 |
   | `amount` | added per level to every harm or mend roll its effects make | 0 |

   Armor and damage are inferred because they are the genre's standard; nothing else is, since a reader cannot tell dark sight grows unless it says so.
   `turns` and `amount` apply to every effect of that kind on the item; an item wanting two statuses growing at different rates is not expressible, and none exists.
3. **Balance is set by `max`, not by a slower armor rate.** Plain armor and helmets `max: 3`, weapons `max: 5`, the nanite and cloak plates `max: 9`. The level table (`levels.ron`) is unchanged.
4. **Heavier weapons grow faster:** the mono-axe and the slug rifle `damage: 2`; every other weapon the default 1.
5. **A pulse's period is its trigger's:** `(on: "pulse", every: 1000, effects: [...])`. `every` is a new field of the engine's `TriggerSpec`, required on a `pulse` trigger and refused on any other.
6. **A charge that refills only while worn is its charges':** Foundry's `consumable: (charges:, when_empty:, recharge: (every: 4000, while_worn: true))`. The top-level `attuned` and `pulse` fields go.
7. **The engine's effects no longer carry `per_level`.** An item's level reaches its effects as an engine component on the carrier, `EffectBonus { turns, amount }`, written by the game when it spawns the thing with the level already applied, as `Bestows`, `Armor` and `Pulse` are. `land_triggers` copies it onto the `Landing` as `bonus`, and `Inflict`, `Harm` and `Mend` add it; `describe` takes the bonus. The engine never learns what a level is.
8. **Grenades are heard through a `Noise` effect**, `(kind: "Noise", args: (sound: "blast", loudness: 14))`, registered by `NoisePlugin`, making one noise at the landing's centre by its user. Frag, ion and incendiary grenades are `blast` at 14; the smoke grenade is `hiss` at 6. Foundry's general `landing` loudness stays 0, so a thrown blade still lands quietly.

### The approaches weighed

- **Scaling written on the number where it is used**, `turns: (base: 10, per_level: 2)`.
  Rejected by the user: it keeps level rules on triggers, which is the scatter this work removes.
- **Inferring every numeric property** (dark sight, pulse) from its presence.
  Rejected by the user: a reader cannot tell dark sight grows per level unless it is written.
- **A slower default armor rate** (+1 per two levels).
  Rejected in favour of per-item `max`: the standard rate is the one a reader expects.
- **Keeping `Landing::level` and `per_level` args, with the game rewriting effect args per level.**
  Rejected: effect arguments are text the engine does not understand; a bonus on the carrier is explicit and keeps levels out of the engine.

## 3. The model, as an item reads

```ron
(name: "rangefinder helmet", slot: "head", tags: ["armor"], armor: 1, dark_sight: 6,
 enchant: (max: 3, dark_sight: 1)),

(name: "mono-axe", ..., melee: (roll: "2d6+1", kind: "kinetic"), cost: 140,
 enchant: (max: 5, damage: 2)),

(name: "nanite plate", slot: "torso", tags: ["armor"], armor: 1,
 enchant: (max: 9, armor: 0, pulse: -100),
 triggers: [(on: "pulse", every: 1000, effects: [(kind: "Mend", args: (kind: "care", roll: "1"))])]),

(name: "cloak plate", slot: "torso", tags: ["armor"], armor: 1,
 enchant: (max: 9, armor: 0, turns: 2),
 consumable: (charges: 1, when_empty: Kept, recharge: (every: 4000, while_worn: true)),
 triggers: [(on: "use", effects: [(kind: "Inflict", args: (status: "cloaked", turns: 10, while_worn: true))])]),

(name: "frag grenade", ..., triggers: [(on: "land", area: Burst(radius: 1), effects: [
    (kind: "Harm", args: (kind: "kinetic", roll: "3d6")),
    (kind: "Noise", args: (sound: "blast", loudness: 14))])]),
```

The nanite plate at `+9` pulses every `1000 - 9 * 100 = 100`, every turn; the cloak plate at `+2` cloaks for 14 turns; a `+3` composite plate has armor 5; a `+2` mono-axe swings `2d6+5`.

## 4. Engine pieces

- `EffectBonus { turns: u32, amount: i32 }`, a component in `crates/rl-bevy/src/effects/`, and `Landing::bonus: EffectBonus` in place of `Landing::level`.
  `Harm` and `Mend` add `amount` to the roll's bonus; `Inflict` adds `turns`; the other effects ignore it.
  `Effect::describe(&self, registries, bonus: EffectBonus)`.
  `per_level` is removed from the three effects' arguments; it was never released.
- `TriggerSpec::every: Option<u32>`, hundredths; `Triggers::build` refuses a `pulse` trigger without it and any other trigger with it, naming the item. The built `Trigger` keeps it so a game can read its carrier's base period.
- `Noise { sound, loudness }`, `FromArgs` kind `"Noise"`, registered by `NoisePlugin`, writing one `MakeNoise` at the landing's `landed_at` (else `aim`) with the landing's user as maker. An unknown sound name is a load error.
- The bag reads a carrier's `EffectBonus` to describe its effects, so a `+2` cloak plate reads `use: cloaked for 14 turns while worn`.

## 5. Foundry pieces

- `ItemDef` loses `pulse`, `attuned` and the old `enchant: (most:)`, and gains `enchant: Option<EnchantDef>` with the keys in section 2; `ConsumableDef::recharge` becomes `Option<RechargeDef { every, while_worn }>`.
- `spawn_item_at` applies the level: `Armor`, the attack rolls' bonus (melee, ranged, throw strike), `WornDarkSight`, `Pulse::every` from the pulse trigger's `every` plus `pulse * level` floored at 100, `EffectBonus { turns * level, amount * level }`, and `Attuned` when `recharge.while_worn`.
- Validation refuses: `enchant` on a thing never worn or on a stack; `dark_sight` in `enchant` without the item's own `dark_sight`; `pulse` without a pulse trigger; `turns` or `amount` on an item with no effects; `max` below 1; `while_worn` recharge without a slot.
- Every weapon and every piece of armor gets an `enchant` block with the `max` in section 2; `item_spawns.ron` and `levels.ron` are unchanged.
- Sounds `blast` and `hiss` are declared; the four grenades gain their `Noise`.
- Saves already keep the level; nothing new is saved.

## 6. Review focus

1. An item at `+0` is exactly what it was before this work: same armor, same rolls, same period, no bonus.
2. A thrown monoblade at `+2` strikes for its throw roll plus 2, as its blow does.
3. A continued run's `+N` items come back with their bonuses, since the level is saved and the bonus is rebuilt from it.
4. A grenade thrown out of sight is heard by droids in range and by the commando's own noise meter.
5. A content file with a level rule in the wrong place (a `per_level` in effect args, `every` on a `use` trigger) fails at load with a message naming the item, never loads quietly.
