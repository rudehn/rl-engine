# Worn gear that does something, the unseen, and enchant by band

Status: design, agreed in conversation on 2026-09-26 against `main` at `f2229bd`.
Nothing here is built yet.

## 1. What this is for

Foundry's armor is a number: plate is armor and a resistance, a helmet is armor and dark sight.
Nothing worn does anything on its own, nothing worn is used, and nothing found deeper is better than the same thing found shallower.
The commando needs gear with a core identity to get further down, and two torso pieces are the first of it:

- a **nanite plate** that mends its wearer one point every ten turns while worn, and
- a **cloak plate** that, used while worn, makes its wearer unseen for ten turns, and then has to charge for forty turns before it can do it again.

Both scale with an enchant level rolled for the depth they are found at.

Done when:

- a worn thing can do something on a clock of its own while it is worn, and can be used only while it is worn;
- a worn thing's clocks run only while it is worn and start over when it is put on, so no swap of gear gets a charge or a mend early;
- an effect landed by an enchanted thing grows with its level, and the bag says what it does at that level;
- an actor holding an `unseen` status is seen by nothing, adjacent included, until it strikes, fires, throws or aims an ability at someone else;
- every place the engine makes an item tells the game the band it is being made at, and Foundry rolls a level for its enchantable things from a table by band;
- a mend at full health says nothing, rather than "You mend for 1.";
- Foundry lays both plates on its decks and in its crates, and both are checked in the running game.

## 2. What was decided

These were settled in conversation and are not reopened here.

1. **An item never lends an ability**, as `docs/design/items.md` section 1 has it.
   The cloak plate is a `use` trigger that inflicts a status, not a granted cloak.
2. **A worn thing is used only while it is worn.**
   A thing that can be worn and carries a `use` trigger is refused, for free, while it sits in the bag.
   The bag does not offer the use key for it.
3. **Attunement is the anti-swap rule.**
   A worn thing's clocks run only while it is worn, and putting it on starts them from nothing.
   For a pulse that is always so; for charges it is so when the thing is `Attuned`, which empties it on being put on and refills it only while worn.
   Wear the nanite plate, swap to the cloak plate: the cloak is empty and forty turns away.
4. **A pulse is a moment.**
   `pulse` is a built-in moment, reported for a worn thing carrying `Pulse` every `every` hundredths of a step it is worn, on its wearer's cell.
   What it does is the thing's `pulse` trigger, landed by the effects subsystem like any other.
5. **An enchant level reaches effects through the landing.**
   `Landing` carries the level of what landed it, which is the carrier's `Enchant` level and zero for anything else.
   `Harm` and `Mend` take `per_level`, added to the roll per level, and `Inflict` takes `per_level`, turns added per level.
   `Effect::describe` takes the level too, so the bag's line is the line at that level.
   What is a number on the thing rather than in an effect, the pulse's period, is written at spawn with the level applied, the way `Bestows` already is.
6. **The nanite plate scales its period, the cloak plate its duration.**
   Nanite: every 10 turns at `+0`, one turn quicker per level, down to every turn at `+9`.
   Cloak: 10 turns at `+0`, two more per level; the forty-turn charge does not scale, because it is what stops the swap.
7. **The band reaches the maker.**
   `ItemMaker::make` takes a `Provenance { found, band }` in place of `Found`.
   The band is the one the draw was asked at: a place's, a region's, a container's plus its row's offset, a prefab slot's plus its offset, and for a drop the band of the place the actor died in.
8. **Levels by band are a table the engine loads and the game consults.**
   `LevelTable` in `rl-rules` holds rows of `(bands, [(level, weight)])`, rolls a level at a band, and draws nothing when a band's row has one level.
   A band no row covers uses the nearest shallower row, else the nearest deeper one, as `LootTable::band_for` does.
   Which things are enchantable, and how high, is the game's: a Foundry item names `enchant: (most: 9)`, and only such items roll.
9. **The unseen is a status property, and stealth answers it.**
   `StatusDef` gains `unseen`; `StealthPlugin` keeps an `Unseen` marker on whoever holds one and makes it mean something.
10. **The rules of the unseen.**
    - No mind perceives an unseen actor at any distance, adjacency included, and no observer notices it.
      This departs on purpose from the stealth rule that no stack of gear makes somebody standing next to you invisible: that rule is about `Stealth`'s quiet, which is permanent, and the unseen is a status that lasts turns and ends the moment you strike.
    - An observer that was alert to it loses it as it would lose anything out of sight: it searches where it last saw it, for its memory, and forgets.
    - Noise is still heard, since hearing is not sight, and a mind that hears something goes to look.
    - Making an attack, a blow or a shot whether it lands or not, throwing anything, or landing an ability on anyone but yourself, ends every unseen status you hold, in the same pass and before its damage lands, so the target wakes to you as it does today.
      The first blow is struck from the unseen: nothing saw it coming.
    - Being hurt does not end it.
    - Radar sees nothing either; whether it should see through the unseen is a future decision (section 9).
    - The vitals strip reads unwatched while you are unseen.
11. **A heal reports what it mended.**
    `DamageDealt::dealt` for a heal is the health actually restored, so a mend at full health reports nought and the narrator says nothing.
12. **Foundry is the first game to use any of it.**
    Two plates, a `cloaked` status with a badge, `levels.ron`, the commando drawn faded while unseen, and the save keeping a thing's level and its pulse.

### The approaches weighed

- **Equip time as the anti-swap cost.**
  Rejected as the rule: time alone still lets a player swap between fights.
  Kept as a future decision, since it is a real cost worth having on top.
- **A shared suit-power pool across worn things.**
  Rejected: a pool is an ability's economy, and an item never lends one.
- **Regeneration as a status held while worn.**
  Rejected: a status ticks every whole turn, and "one every ten" is a period, which a status has no way to say.
  A pulse is the general form and a status held while worn remains the separate idea `docs/design/items.md` section 6 names.
- **Scaling effects by rewriting their RON arguments per level at spawn.**
  Rejected: the arguments are text the engine does not understand, and a generic numeric rewrite would silently skip a field that is not a number.
  A level on the landing is explicit, and a game's own effect reads it the same way.
- **Enchant levels rolled inside the engine.**
  Rejected: which things are enchantable and how far is content the engine never sees; the engine hands the band and the stream and owns the table's arithmetic.
- **The band inside `Found`.**
  Rejected: `Found` says why and is compared by games as a plain value; a band beside it is a second fact, so both travel in `Provenance`.
- **A separate plugin for the unseen.**
  Rejected: it is a question stealth already asks, who can see whom, and splitting it would make a game add two plugins for one idea and order their filters by hand.
- **Smoke showing an unseen outline.**
  Dropped: smoke already veils sight, so there is nothing to see an outline through.

## 3. Worn things: the pulse and attunement

- `Moments::PULSE` is `MomentId::from_raw(6)`, `"pulse"`, appended to `Moments::BUILT_IN`, so every existing moment keeps its id.
- `Pulse { every, progress }` in `crates/rl-bevy/src/items.rs`: hundredths of a step per pulse, and hundredths counted towards the next.
- `pulse_worn` runs in `ResolveSet::Triggers` before `land_triggers`, reads the clock's advance since its last run, and for each wearer on the current map adds it to each worn `Pulse`, writing one `Fired` for each full period, on the item, by the wearer, at the wearer's cell.
  Wearers elsewhere are skipped: `land_triggers` finds its targets by occupancy on the current map, and a pulse on another map would land on whoever stands on the same coordinates here.
- `restart_pulses` runs in `TurnSet::React` and sets `progress` to zero on every `ItemEvent::Equipped`.
- `Attuned` in `crates/rl-bevy/src/consumable.rs` marks a `Consumable` whose charges come back only while worn.
  `recharge_charges` skips an attuned thing no wearer has on, and `attune` runs after it in `TurnSet::React` and empties an attuned thing, charges and progress both, on every `ItemEvent::Equipped`.
- `UseItem` of a `Wearable` thing that is not in the user's `Equipped` is refused and costs nothing.
- `EffectState` in `rl-save` keeps a `Pulse`'s progress beside a consumable's.
- The bag: a pulse trigger's line reads `every 10 turns worn: mends 1 care`; a thing that refills and is charging while its clock runs says `ready in N turns` in place of `empty`; an attuned thing off the body says `charges only while worn`; `ItemRow::usable` is false for a wearable thing not worn.

## 4. Effects at a level

- `Landing::level: i32`.
  `land_triggers` reads it off the carrier's `Enchant`; an ability's landing and `Effects::land_on` pass zero.
- `Effect::describe(&self, registries: &Registries, level: i32) -> String`, and `Effects::describe(registries, level)`.
  This is a breaking change for a game's own effect, which adds the parameter; the CHANGELOG says so.
- `Harm { kind, roll, per_level }` and `Mend { kind, roll, per_level }`: `per_level: i32`, default 0, added to the roll's bonus per level.
- `Inflict { status, turns, per_level }`: `per_level: u32`, default 0, turns added per level.
- The bag describes a thing's triggers at the thing's own level.

## 5. Band and level

- `Provenance { found: Found, band: i32 }` in `crates/rl-bevy/src/loot.rs`, and `ItemMaker::make(&self, commands, registries, def, count, from: Provenance, rng)`.
- `lay` takes a `Provenance`; `scatter_places`, `scatter_regions`, `fill_containers`, `fill_prefabs` and `drop_on_death` each pass the band they drew at.
- `rl_rules::loot::{LevelRow, LevelTable, load_levels}`:
  - `LevelRow { bands: (i32, i32), levels: Vec<(i32, u32)> }`;
  - `LevelTable::new(rows) -> Result<Self, Vec<String>>`, refusing an empty level list, a weight of nought, a negative level, a backwards band range and two rows that share a band;
  - `LevelTable::row_for(band)` and `LevelTable::roll(band, rng) -> i32`;
  - `load_levels(text) -> Result<LevelTable, ContentError>`.
- Corsair keeps rolling by `Found` through `from.found`.

## 6. The unseen

- `StatusDef::unseen: bool`, the builder `StatusDef::unseen()`, and `unseen: true` in a status file.
- In `crates/rl-bevy/src/stealth.rs`:
  - `Unseen`, a marker, kept by `mark_unseen` in `ResolveSet::Effects` after `tick_statuses`, from `Changed<Afflicted>`;
  - `reveal_attackers` in `ResolveSet::Effects` before `resolve_afflictions`, reading `Struck`, `ItemEvent::Thrown` and `AbilityEvent::Used` with a target other than the user, and writing a `Cure` for each unseen status the attacker holds;
  - `filter_unseen` in `PerceiveSet::Filter` before `filter_unnoticed`, taking the unseen out of a mind's enemies, allies and others;
  - `update_awareness` counts an unseen subject as out of view;
  - `Watchers::sees` and `Watchers::watched` answer false for an unseen subject.
- `StealthPlugin` registers `Cure` and declares that it reads `Struck`, `ItemEvent` and `AbilityEvent`, so a game without combat, items or abilities has nothing to reveal on rather than a panic.

## 7. Foundry

- `content.rs`: `StatusDef { badge: Some('%'), ..StatusDef::new("cloaked").unseen() }`.
- `items.ron`, two new rows and three new fields, each in the schema comment at the top:
  - `pulse: (every:, per_level:, fastest:)` hundredths per pulse at `+0`, added per level, and the shortest period any level reaches;
  - `attuned: true` for a thing whose charges come back only while worn;
  - `enchant: (most:)` for a thing that rolls a level where it is found, at most `most`.
- `nanite plate`: torso, armor 1, `pulse: (every: 1000, per_level: -100, fastest: 100)`, `enchant: (most: 9)`, a `pulse` trigger mending 1 `care`.
- `cloak plate`: torso, armor 1, `attuned: true`, `consumable: (charges: 1, when_empty: Kept, recharge: 4000)`, `enchant: (most: 9)`, a `use` trigger inflicting `cloaked` for 10 turns with `per_level: 2`.
- `item_spawns.ron`: the nanite plate on decks 3 to 10 and the cloak plate on 1 to 10, weight 1 each; both carry `armor`, so a store crate's armor row draws them too.
- `levels.ron`, loaded into the `Armory`:

  | Decks | Level: weight |
  | --- | --- |
  | 1-2 | 0: 9, 1: 1 |
  | 3-4 | 0: 6, 1: 3, 2: 1 |
  | 5-6 | 0: 3, 1: 4, 2: 2, 3: 1 |
  | 7-8 | 0: 1, 1: 3, 2: 3, 3: 2, 4: 1 |
  | 9 | 1: 2, 2: 3, 3: 3, 4: 2 |
  | 10 | 2: 2, 3: 3, 4: 3, 5: 2 |

  A `+1` is an uncommon surprise from deck one, one find in ten, and `+5` is found only at band 10: on the last deck, or in a container on a shallower deck whose row asks two bands down, such as a guard post's locker on deck eight.
  The table reaches `+5`; `most: 9` is there so the nanite plate's curve is written to its end, every turn at `+9`, for whatever later raises a thing past what the decks hand out.

- The `Armory`'s maker rolls a level for each enchantable thing it makes, at `from.band`, capped at the thing's `most`, from the stream it is handed, and spawns it named `cloak plate +2` with `Enchant` on it.
- `ItemSave` keeps the level; a continued run respawns the thing at it.
- The commando is drawn faded while unseen and in white otherwise.

## 8. The review focus this implies

1. A pulse on a wearer who is not on the current map lands on nobody, not on whoever stands at the same coordinates here.
2. Putting the cloak plate on, taking it off and putting it on again leaves it empty each time, however much it had charged.
3. A thrown grenade and an ability aimed at a foe end the unseen exactly as a blow does; a stim does not.
4. A band past the level table's last row rolls from the last row, not plain.
5. A save taken mid-charge and mid-pulse continues with the same charge, progress and level.

## 9. Future decisions

- **Does radar see the unseen?** An ion grenade already blinds radar; if radar sees through a cloak, the pair is a combo.
- **Walking into the unseen.** A searching mind that steps into an unseen actor's cell is refused the step and learns nothing today.
- **An unseen non-player on the player's screen.** Nothing hides a cloaked droid from the map yet; the stealth design names this as a render slice.
- **Sneak-attack damage** from the unseen, which the stealth design already defers.
- **Equip time per item**, on top of attunement.
- **Enchantable plates, helmets and weapons**, and what a level buys each.
- **More effects that scale**: `Shove`, `Pull`, `Emit`, `Ignite`.
