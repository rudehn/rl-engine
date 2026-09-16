# Abilities

Status: phases A to F built 2026-09-12.
Written against `main` at `8662f1c`, with the UI slice (modals, views, panels, the look cursor) in flight.
It closes the "abilities and targeting" item deferred from M5.

Four things changed on the way from this document to the code, each noted where it happens below: the effect that puts a status on is `Inflict`, not `Afflict`, because `Afflict` is already the request and an effect is not a request; names in an ability file are resolved through [`Names`], the engine's one lookup over whichever registries a game has, which was not in the plan and removed the per-game mirror types the content layer used to need; a cooldown counts from the moment of use rather than the end of it; and the sight requirement is not applied to an actor with no viewshed, since most non-players carry none and the question cannot be asked of them.

Phase D changed four more.
A key needs no aiming code at all: a game writes `AimAt` and the engine opens the cursor, or uses a self ability at once, so every ability is bound the same way.
`Offered` covers whoever holds the turn rather than only a deciding mind, and gained the refused list with its reasons, so the minds, the menu and the preview all read one answer from one gate.
The targeting view counts a target that has no `Name` or `Glyph`, where the other views leave such a row out, because a nameless row is one a list may skip but a nameless target is one the ability will hit anyway; the first run of Knacks said "fireball at nothing" over a brute for exactly this reason.
And `rl-grid` became a real dependency of `rl-ui` rather than a dev-only one, since previewing a footprint is grid work.

Revised 2026-09-13: one answer to where a use lands.
The resolver, the preview and the ability tactic each worked out the footprint and who stood in it, and they had drifted four ways apart: the user was skipped though `Aim::Ally` includes it, the preview refused a footprint with no cells that the resolver paid for, the preview listed blockers with no health that the resolver passed by, and the tactic counted allies as harmed by a foe-aimed burst that never touches them.
Knacks showed the first as "medspray at nothing" over a hurt player, and the second as twelve mana spent on a fireball the banner had called refused.
Now `Aim::hits` and `Aim::worth_aiming_at` in `rl-rules` are the rules, `aim_blocked` the aim's own refusals, and `Bystanders::land` in `rl-bevy` the one call the resolver lands a use with and the cursor previews one with, held to it by a property test over seeded layouts.
`rl-grid` went back to a dev-only dependency of `rl-ui`, since the cursor no longer resolves anything itself.
The same run found that no heal had ever landed: `resolve` clamped every hit at zero, and a mend is a negative one. The clamp moved to where blows are rolled.

## 0. Summary

An ability is the second thing an actor can spend a turn on.
The first is an attack, which the engine already owns end to end: a target, a shape, a cost, a roll, a mitigation pipeline, an event.
An ability is that same sentence with the parts named by data instead of hard-coded, so a game can write a hundred of them in RON and never touch Rust.

`rl-grid::targeting` has shipped the shapes since M3 and nothing uses them but `RangedAttack`.
That is the failure mode section 1 of `PLAN.md` names: data structures shipped, behaviour left in the game.
This slice owns the loop.

Three decisions shape everything else.

1. **Effects are types, not a list.**
   The same answer actions got in `10e3d82`, one level down.
   The engine ships an effect per subsystem it owns, all in `effects`, and a game registers its own with `add_effect`.
   There is no `Custom { id }` variant and no enum of effect kinds anywhere.
2. **Fuel is a stat, not a resource type.**
   The engine never learns the word mana.
   A cost is an amount against a registered [`StatId`], an item charge, health, or an item carrying a tag.
   Mana, stamina, power cells, nerve, heat, powder and adrenaline are all the first of those, and the engine cannot tell them apart.
3. **A mind must be able to use one, or the subsystem is half-built.**
   The engine cannot know what an ability means, so a definition declares its [`Aim`]: the kind of thing it wants under its footprint.
   That single field is enough for a tactic to pick a target and a score without ever knowing the theme.

## 1. What abilities buy

Roughly in the order they become cheap.

- **Anything a wand, a grenade, a cannon, a taser or a smoke bomb does.**
  One `Use` action, one RON entry each.
- **Monsters with more than a bite.**
  A tactic that fires an ability turns a bestiary of stat blocks into a bestiary of behaviour, at no cost per monster kind.
- **Scripted encounters**, the other M5 deferral.
  An encounter is an ability with no user: the same effect list, triggered by a fact rather than a turn.
- **Items that do something interesting.**
  Today a consumable is a game's `use_items` reaction.
  With abilities, a potion, a grenade and a medkit are the same machinery with different RON, and the game writes no system at all.
- **The targeting cursor**, which every roguelike writes and every one gets wrong the first time.
  It shares its movement, its cycling and its modal with the look cursor already in flight.
- **Cooldowns and charges as engine state**, which means they are saved, ticked by the same integer clock, and visible to a panel.
- **Later, cheaply: fire and gas.**
  A tile field is an effect that writes cells rather than damage.
  The ability layer is the thing that would place one.

## 2. The five genres, in one table

The point of the design is that nothing in tiers 0 to 2 changes between these rows.
Everything in "cost", "shape" and "effects" is RON.

| Game | Ability | Shape | Cost | Effects |
| --- | --- | --- | --- | --- |
| Fantasy | Fireball | `Ball { range: 8, radius: 2 }` | `Pool(mana, 12)` | `Harm(fire, 6d6)` |
| Fantasy | Blink | `Ground`, range 6 | `Pool(mana, 5)`, cooldown 800 | `Teleport` |
| Pirates | Broadside | `Cone { length: 5 }` | `Item(tag powder, 1)` | `Harm(pierce, 3d6)`, `Afflict(deafened)` |
| Pirates | Grapnel | `Bolt { range: 7 }` | cooldown 500 | `Pull(3)` |
| Sci-fi | Overload | `Bolt { range: 9 }` | `Pool(power, 8)` | `Harm(shock, 4d6)`, `Afflict(stunned, 2)` |
| Sci-fi | Cloak | `SelfOnly` | `Pool(power, 15)`, cooldown 2000 | `Afflict(cloaked, 10)` |
| Medieval | Shield bash | `Adjacent` | `Pool(stamina, 6)`, requires `InSlot(offhand, tag shield)` | `Harm(blunt, 1d6)`, `Shove(1)`, `Afflict(dazed)` |
| Medieval | Rally | `Cone { length: 4 }`, `Aim::Ally` | `Pool(stamina, 10)`, cooldown 1500 | `Afflict(inspired, 8)` |
| City crime | Shiv | `Adjacent` | requires `Lacks(spotted)` | `Harm(pierce, 2d4)` at triple |
| City crime | Bribe | `Adjacent`, `Aim::Anyone` | `Item(tag cash, 50)` | `Bribe` - the game's own |
| City crime | Smoke bomb | `Ball { range: 5, radius: 2 }`, `Aim::Ground` | `Charge(1)` | `Kindle(dark)`, `Afflict(blinded, 4)` |

Nine of those eleven are built out of effects the engine ships.
`Bribe` is not, and that is the honest part of the design: flipping one actor's relation to the player is game vocabulary, the engine has no business knowing it, and the game writes forty lines to add it.
The measure of the design is that those forty lines are an `impl Effect` and a registration, not a fork of the resolver.

## 3. The model

### 3.1 The definition

In `rl-rules/src/ability.rs`, tier 1, no Bevy.

```rust
pub struct AbilityDef {
    pub name: String,
    /// What it wants under its footprint, so a mind can choose a target.
    pub aim: Aim,
    /// The shape, from rl-grid. Range lives inside the shape.
    pub mode: TargetMode,
    /// Whether the aim must be a cell the user can actually see. False for
    /// a grenade over a wall.
    pub needs_sight: bool,
    /// Everything that must be true of the user.
    pub requires: Vec<Requirement>,
    /// Everything spent on a use, all or nothing.
    pub costs: Vec<Cost>,
    /// What the turn costs, in hundredths of a step, like every other clock.
    pub time: u32,
    /// Hundredths before it may be used again. Zero is no cooldown.
    pub cooldown: u32,
    /// What lands, in order, each with a percentage chance.
    pub effects: Vec<EffectSpec>,
}
```

`EffectSpec` is a name, a chance, and the raw RON the effect will parse:

```rust
pub struct EffectSpec {
    pub kind: String,
    pub chance: u8,
    pub args: ron::Value,
}
```

Untyped until load, and validated there: a typo in an effect's arguments fails at startup naming the ability and the effect, not at the moment a player presses the key.
That is the same bargain `Registry::validate` already makes everywhere else.

### 3.2 `Aim`

```rust
pub enum Aim {
    /// The user's own cell, and no cursor.
    SelfOnly,
    /// Something the relation matrix calls hostile.
    Foe,
    /// Something it calls friendly, the user included.
    Ally,
    /// A cell, whoever is standing on it.
    Ground,
    /// Any actor.
    Anyone,
}
```

A closed enum, deliberately, and the reason is worth stating because the plan forbids closed taxonomies for content.
This is not content.
It enumerates the questions the engine's own faction matrix can answer about a cell, the same way `TargetMode` enumerates the shapes its own geometry can draw.
Adding a sixth kind of thing to aim at would mean the relation matrix had grown a sixth answer.

`Aim` is what lets a mind fire an ability it cannot understand: score a footprint by how many foes it covers, or how many hurt allies, and the tactic never asks what the ability does.

### 3.3 Costs

```rust
pub enum Cost {
    /// An amount off a registered stat: mana, stamina, power, nerve, heat.
    Pool { stat: StatId, amount: i32 },
    /// A charge off whatever granted the ability.
    Charge { amount: u16 },
    /// Health, for the abilities that should hurt to use.
    Health { amount: i32 },
    /// Items in the bag carrying a tag: a powder charge, a reagent, cash.
    Item { tag: TagId, count: u16 },
}
```

Closed for the same reason: it enumerates the things the engine can already decrement, each of which is a subsystem it owns.
A game that wants a sixth kind of fuel registers a stat and uses `Pool`.
`Pool` covers every genre's fuel because the engine never learns what the stat is called.

Costs are all-or-nothing and paid once, after the gate and before the effects, so a refused ability is free and a landed one cannot double-charge.

### 3.4 Requirements

```rust
pub enum Requirement {
    Has(StatusId),
    Lacks(StatusId),
    Wielding(TagId),
    InSlot(SlotId, TagId),
    Above(StatId, i32),
}
```

Each reads a table the engine already owns: the status set, the slot graph, the stat block.
This is where a shield bash learns it needs a shield and a shiv learns it needs to be unseen, without either concept entering the engine.

### 3.5 Effects are types, not a list

The trait lives in `rl-bevy`, because applying an effect means writing to the world.

```rust
pub trait Effect: Send + Sync + 'static {
    /// What lands, once, on a resolved footprint.
    fn apply(&self, landing: &Landing, world: &mut EffectWorld);
}
```

`Landing` is the resolved use: the user, the ability, the aim point, the footprint's cells and path, and the living actors standing in it that `Aim::hits`, the user counted as its own ally.
It comes from `Bystanders::land`, which the targeting preview calls too.
`EffectWorld` is a `SystemParam` exposing the requests the engine already answers - `DamageEvent`, `Afflict`, `Cure`, a teleport, a spawn, a light - plus `Commands`.
`Commands` is the escape hatch, and it is a real one: an effect a game writes can do anything a system can do, including write its own messages.

Registration mirrors actions exactly:

```rust
app.add_effect::<Harm>()      // the engine's, in effects.rs
   .add_effect::<Bribe>();    // the game's, in its own module
```

`add_effect::<E>` records a constructor `fn(&ron::Value) -> Result<E, String>` under `E`'s name in the `EffectKinds` resource.
Loading an ability file turns every `EffectSpec` into a `Box<dyn Effect>` once, at load, so nothing re-parses RON during play and an unknown effect name is a startup failure that names the ability.

The engine ships one effect per subsystem, all in `effects.rs`, and each asks the subsystem that owns the mechanic:

| Effect | Asks | What it asks for |
| --- | --- | --- |
| `Harm { kind, roll }` | combat | a `DamageEvent` per actor in the footprint |
| `Mend { kind, roll }` | combat | negative damage, through the same pipeline, past armor and scaled by resistance |
| `Inflict { status, turns }` | statuses | an `Afflict` per actor |
| `Cleanse { status }` | statuses | a `Cure` per actor |
| `Shove { cells }` / `Pull { cells }` | `EffectWorld` | a move along the line, stopping at a blocker |
| `Teleport` | `EffectWorld` | the user to the landing cell, if it is free |

They began in the modules they ask, `Harm` in combat and `Inflict` in statuses, which made combat and statuses depend on abilities to implement `Effect`.
In one module the dependency runs the way the design does: abilities are built on combat and statuses, and combat reads, and is tested, without a word about them.

Seven, not the nine first sketched: `Kindle` and `Summon` land with the slices that need them, since neither Knacks nor Corsair asks for one yet, and `Grant` turned out to be a `Known` rebuilt from a status rather than an effect of its own. The three movement effects sit beside the resolver rather than in the turn loop, because the move goes through `EffectWorld::slide`, which is what keeps a shove out of a wall.

That list is closed only in the sense that the engine's subsystems are.
It grows when a subsystem does, which is the right coupling.

### 3.6 What is data, and what is not

An ability is data.
The vocabulary an ability is written in is code.

Everything in an `AbilityDef` comes out of RON and nothing about it is known at compile time: the shape and its range, what it aims at, what it costs, what it requires, what it takes off the clock, how long before it can be used again, which effects land, in what order, at what chance, with what arguments.
Every id inside it is a name resolved against a registry at load - a stat, a status, a tag, a slot, a damage kind, another ability.
So adding an ability is editing one file, and a game may ship five hundred without compiling anything.

```
// A game's abilities: what an actor can spend a turn on besides a step
// and a swing.
//
// Every field (the ones marked "optional" may be left out):
//   name:      unique, referred to by items' `grants`, by affixes and by monsters
//   aim:       what it wants under it: Foe (default) | Ally | SelfOnly | Ground | Anyone
//   mode:      the shape: Own | Adjacent | Bolt(range) | Ball(range, radius) |
//              Beam(range) | Cone(length)
//   sight:     optional; whether the aim must be a cell the user can see (default true)
//   requires:  optional; Has(status) | Lacks(status) | Wielding(tag) |
//              InSlot(slot, tag) | Above(stat, n)
//   costs:     optional; Pool(stat, n) | Charge(n) | Health(n) | Item(tag, n)
//   time:      optional; hundredths of a step the turn costs (default 100)
//   cooldown:  optional; hundredths before it may be used again (default 0)
//   effects:   each (kind, chance, args); kind is any effect the game registered,
//              engine or its own: Harm | Mend | Afflict | Cleanse | Shove | Pull |
//              Teleport | Kindle | Summon | Grant
#![enable(implicit_some)]
[
    (
        name: "broadside",
        mode: Cone(5),
        costs: [Item("powder", 1)],
        cooldown: 400,
        effects: [
            ("Harm", 100, (kind: "cannon", roll: "3d6")),
            ("Afflict", 40, (status: "deafened", stacks: 3)),
        ],
    ),
    (
        name: "swig",
        aim: SelfOnly,
        mode: Own,
        costs: [Item("rum", 1)],
        effects: [
            ("Mend", 100, (roll: "2d4")),
            ("Cleanse", 100, (status: "bleeding")),
            ("Afflict", 100, (status: "hearty", stacks: 20)),
        ],
    ),
]
```

Neither of those needs a line of Rust.
Nor would a fireball, a taser, a smoke bomb or a rally, because each is `Harm`, `Afflict`, `Kindle` or `Mend` over a shape.

What is not data is the set of verbs.
`Harm` is a Rust type, because something has to know that a damage roll becomes a `DamageEvent` per actor in a footprint and goes through the mitigation pipeline.
Making that step data too would mean inventing an expression language, an interpreter and a debugger for it, and the engine would have shipped a scripting runtime rather than a roguelike engine.
Games that go that way end up writing the interesting half of the game in a language with no types, no tests and no stack traces.

So the boundary is drawn at the verb:

| Adding | Costs |
| --- | --- |
| An ability | A RON entry |
| A variation on one: another range, cost, chance, status, order | A RON edit |
| A new kind of fuel | A stat in the stat file, then RON |
| A new verb the engine does not have: `Bribe`, `Hack`, `Possess` | An `impl Effect` and one `add_effect` line, then RON forever after |

The third row is the one that matters, and it is deliberately cheap: an effect is a struct that deserializes its arguments and a function that writes some requests.
Once a game has written its five, its designers are back in RON.

## 4. Where each piece lives

### rl-rules: `ability.rs` (tier 1, no Bevy)

Names in an ability file are resolved through [`Names`], a value borrowing whichever of the stat, status, tag, slot and damage kind registries a game has.
This was not in the plan, and it is the difference between abilities being data and being nearly data.
It began as a trait each game implemented, and every implementation turned out to be the same five one-line methods over the same five registries, so the engine owns the one copy.
Statuses and affixes load through it too: they used to make a game mirror the engine's schema in a RON-facing type of its own and convert, which Corsair did in a hundred lines, and now a game hands `Names` to `status::load` and `affix::load` and authors both by name.


`AbilityDef`, `Aim`, `Cost`, `Requirement`, `EffectSpec`, and the pure decisions over them:

- `gate(def, &Gates) -> Result<(), Blocked>` - every requirement checked, every failure collected, so a panel can print all of them.
- `affordable(def, &Purse) -> Result<(), Blocked>` and `pay(def, &mut Purse)`.
- `Blocked`, the reason: an unmet requirement, an unaffordable cost, a live cooldown, no target.

`Gates` and `Purse` are small borrowed structs the caller fills from its components, the way `forecast::Combatant` already is.
All of it is testable without a window.

### rl-bevy: `ability.rs` (tier 2)

- `Abilities`: the registry plus the boxed effects, built at load.
- `Known(Vec<AbilityId>)`, `Pools(Stats)` - reusing `StatBlock` rather than a second store - `Cooldowns(BTreeMap<AbilityId, u32>)` keyed on the same clock the turn queue runs on, and `Charges(u16)` on the granting item.
- `Use { ability: AbilityId, aim: Point }`, an `Action` like any other, resolved in `TurnSet::Resolve`.
- The resolver: gate, afford, resolve the footprint against the map and occupancy, pay, run the effects in order against `AbilityRng`, set the cooldown, and report through `Resolution`: `done` with `def.time`, or `failed` with it when the gate or the aim refuses.
  A refusal costs nothing and keeps the turn, for the player only, exactly as the attack resolver does.
- `AbilityEvent { user, ability, cells, landed }` for narration and for facts.
- `AbilitiesPlugin`, opt-in, declaring what it needs the way the other plugins now do.

Effects run inside `Resolve`, not `React`, so the damage they cause goes through the existing pipeline in the same pass and a game's reaction sees it on schedule.

### rl-ui: the target cursor

The look cursor already opened onto the nearest thing worth looking at, stepped with `DirectionKeys`, cycled with Tab, and held a modal.
What the two cursors share is their keys and how a key moves them, so `cursor.rs` holds `CursorKeys`, the one set of bindings, and `steer`, the one reading of a frame's keys: close, confirm, cycle to the next candidate nearest first, or step without leaving the loaded window.
Each cursor keeps only where it is and what it is for.
It began as three shared functions with a key resource per cursor, and the two systems had grown the same close, next and step handling twice.
`TargetView` carries the ability, the cursor, the footprint and targets from `Bystanders::land`, the call the resolver lands the use with, whether the aim is legal, every reason it is not, and a row per target.
The cursor opens on what `Aim::worth_aiming_at` picks, the rule the tactic aims by.
`TargetPanel` repaints the backgrounds the map already drew rather than drawing a box: the cells hit, the flight to them, and, when the resolver would refuse, the whole footprint in the bad tone with the reason in its banner.
`AbilityView` lists what the turn-holder knows in registration order, so a key bound to the third row stays bound to it, with the gate's reasons; `AbilityPanel` draws it with the existing list menu under a modal of its own.

### rl-render

The footprint drawn as a tint over the map view, in `PresentSet::Overlay`: one colour for the cells that will be affected, another for the flight path, a third when the aim is illegal.
No new machinery; `shade` already composes a tint per cell.

## 5. Where an actor's abilities come from

`Known` is rebuilt, not edited, by a system in `TurnSet::React`, the way gear modifiers already are.
Four sources, in order:

1. The actor's own definition, from the bestiary.
2. Equipment: a wand, a pistol, a cyberdeck, a boarding axe. The item carries `Grants(Vec<AbilityId>)` and, if it is limited, `Charges`.
3. Affixes, through the existing `AffixDef`, so "of flame" can grant an ability as easily as it grants a stat.
4. Statuses, so a drug, a possession or a battle trance can lend an ability for a duration.

Rebuilding rather than editing means an unequipped wand takes its ability with it and nothing has to remember that it did.
`Stats::retain_sources` already solved the same problem for modifiers; this is the same shape.

## 6. Minds

A new tactic in `rl-rules::ai::tactics`, sitting wherever the game's brain puts it.

The engine filters first, in tier 2: before the brain runs, the actor's usable abilities are narrowed to those that pass the gate, are affordable and are off cooldown, and the survivors are handed to the snapshot as `(AbilityId, Aim, TargetMode)`.
The tactic then scores each against what the actor can see - foes covered for `Aim::Foe`, hurt allies for `Aim::Ally`, self for `SelfOnly` - and returns the best, or nothing.
Who a footprint catches is `Aim::hits` and what is worth catching `Aim::worth_aiming_at`, the rules the resolver lands the use with, so a hurt caster points a heal at itself and a foe-aimed burst over an ally costs the mind nothing.

`Decision` gains one variant, `Ability { id, aim }`, for the same reason `Attack` is one: it is an action the engine owns and resolves.
Nothing about a game's own choice changes, and a game's own ability-like tactic keeps working through `Decision::Own`.

Refusing to build this is refusing to build the subsystem.
A wand the player can fire and a monster cannot is the half-shipped shape `PLAN.md` section 1 is about.

## 7. Save, determinism and facts

- Cooldowns and pools go in the save beside the scheduler's clock, in the same units.
  Built 2026-09-13, after the review found this promised and not done: `EngineSave` holds them, with the charges on whatever lends an ability, for every entity the game saved.
  A cooldown is an absolute time on the turn queue's clock, not a countdown, so restoring the clock restores every cooldown for free.
- `AbilityRng` is derived from the run's `Seed` on its own domain, by the plugin rather than the game, so adding an ability does not shift the combat stream and change every monster's rolls.
- Chances are rolled once per effect per use, in order, from that stream.
- An ability used is a `FactKind`, so a quest can require one: "hex the idol", "blow the door", "bribe the watch".
  It reports the user as subject, the ability as object, and the number of actors it landed on as the amount.

## 8. Phases

Each phase compiles, passes, and is worth committing alone.

**A. The definition and the gate** (`rl-rules`). Built.
`AbilityDef` and friends, `gate`, `affordable`, `pay`, `Blocked`, RON loading with validate-on-load.
No effects, no Bevy, no example.
Tests are pure.

**B. The action and the engine's effects** (`rl-bevy`). Built.
`Abilities`, `Known`, `Cooldowns`, `Use`, the resolver, `Effect`, `EffectWorld`, `add_effect`, and `Harm`, `Mend`, `Afflict`, `Cleanse`, `Teleport`.
Fired from a test with a literal aim point; no UI yet.
This is the phase that proves the loop is owned.

**C. Grants and charges.** Built, less the move of Corsair's pistol, which is a change to Corsair rather than to the engine and waits for a slice of its own.
`Grants` on items, affixes and statuses; `Charges`; the `React` rebuild of `Known`.
Corsair's pistol moves from a bespoke key to an ability, and `f` becomes the ability key.
The first proof that a game loses code.

**D. The cursor and the menu** (`rl-ui`). Built.
`cursor.rs` shared with the look cursor, `TargetView` and `TargetPanel`, `AbilityView` and `AbilityPanel`.
Knacks lost its stand-in aiming code: an ability key writes `AimAt` and stops.

**E. Minds.** Built.
The tactic, the snapshot's usable list, `Decision::Ability`.
The delve's warden gets a breath weapon and stops being a sack of health.

**F. The genre proof.** Built.
A fourth example, `examples/knacks`: one small map, five ability sets in five RON files, a key to switch between them, and one game effect per genre that the engine does not ship.
All five load into one registry, which is sharper than the plan: a fireball and a smoke bomb are two rows of one table, and switching sets is a change of grants and nothing else.
It is the test that the seams are right, in the same way Corsair is, and it is what makes the answer to "will this work for my game" a command rather than an argument.
Folded on 2026-09-13: the pirates set went to Corsair with `Plunder` spilling a purse rather than moving a coin, the fantasy and medieval sets to the delve with `Drain`, and the claim that five genres share one registry became `crates/rl-bevy/tests/genres.rs`.

The remaining engine effects - `Shove`, `Pull`, `Kindle`, `Summon`, `Grant` - land wherever the phase that needs them does.

## 9. Tests

- Pure, in `rl-rules`: an ability with an unmet requirement is blocked and every unmet requirement is listed; costs are paid exactly once or not at all; a cost that cannot be met leaves the purse untouched.
- Over a seed range: an ability with `needs_sight` never lands on a cell outside the user's viewshed, across generated maps.
- Over a seed range: no ability is used twice inside its cooldown, at any speed, including speeds that give two actions per turn.
- A refused ability costs the player no time and keeps the turn; a mind is never handed an ability it cannot afford, so it cannot loop.
- A save roundtrip restores cooldowns and pools, and a cooldown set before saving is still live after loading at the same clock.
- Fingerprint tripwire, labelled as such: a fixed seed, a fixed ability, a fixed footprint, and the exact damage events it produces.
- Content: the five genre files load, validate, and refuse a file with an unknown effect name, an unknown status and a negative cost, naming all three.

## 10. Cost

Phase A around 350 lines with tests, B around 600, C around 200, D around 400 sharing the cursor, E around 200, F around 500 counting RON.
Roughly 2,300 lines, of which the example is a fifth.
Corsair and the delve should each lose code on C and E.

## 11. Risks, and what is left out

- **`EffectWorld` becoming a god parameter.**
  The discipline is that it exposes messages and `Commands`, never component queries, so an effect asks for an outcome and never performs one.
  If an effect needs to read the world, it writes a request and a system answers it.
- **`ron::Value` in a definition.**
  Untyped arguments are the price of an open effect set without a `Custom` variant.
  Validate-on-load pays most of it: a bad argument fails at startup, naming the ability.
- **The cursor's dependency on the UI slice.**
  Resolved: the slice landed first, and phase D built on its modals, views and look cursor without changing any of them beyond moving the look cursor onto the shared arithmetic.
- **Accuracy does not exist.**
  Abilities land unconditionally, like every melee attack today.
  When a to-hit roll arrives it is a stage in the damage pipeline, not a change here.
- **Not built: ability trees, levelling, schools, spell failure, counterspelling, casting interrupted by damage.**
  Every one of them is a game's rule over the data this slice provides, and the engine should not guess which.
- **Not built: an ability with no user.**
  Scripted encounters want the effect list without the turn, the cost and the cursor.
  That is a small addition once effects exist, and it belongs to the encounter slice, not this one.
