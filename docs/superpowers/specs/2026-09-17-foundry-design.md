# Foundry: design

A tenth-deck descent into a droid foundry, as the engine's fourth worked example.

Written 2026-09-17.
Status: design agreed, not yet planned.

## 1. What this is, and why

`examples/foundry` is a new example game for `rl-engine`.
It exists for three reasons, in order.

1. It is the example that exercises the engine's combat depth: three damage kinds with a real counter-triangle, eighteen weapons across melee and ranged, four armor slots, and a loot curve over ten floors.
2. It drives four small engine additions, listed in section 10, each of which the engine's own documents already say is missing or which a game cannot do without copying engine logic.
3. It is a science-fiction example beside a pirate one and two dungeons, which is the evidence that the engine carries no genre.

It is expected to move to its own repository once it is playable.
Everything in this design keeps that move cheap: the game owns its content, its systems and its vocabulary, and it depends on the engine only through published seams.

## 2. Setting

A Separatist droid foundry sunk into an asteroid, ten decks deep.
The player is a clone commando, alone, after the rest of the squad's transport was hit on approach.
The mission is sabotage: charges on three reactors, then the core.

Droids thicken as the player descends, which is both the difficulty curve and the reason the loot is scavenged rather than bought.
A bounty hunter syndicate is in the foundry for its own reasons and is hostile to the player and to the droids.
Foundry vermin live in the coolant ducts and are hostile to everyone.

### 2.1 Names, and the intellectual property line

The Star Wars vocabulary is not shipped.
Every proper noun lives in the game's RON display names, and the Rust code and every content id stay neutral.

- Rust and ids: `line_droid`, `heavy_droid`, `shield_droid`, `guard_droid`, `infiltrator`, `commando`.
- The public asset pack, `assets/`: "line droid", "heavy droid", "shield droid", "guard droid".
- A private asset pack, not committed: the Star Wars names, for the author's own runs.

This is the same rule the engine applies to itself one layer up, and it means swapping vocabulary is a directory rather than a refactor.
No file in `crates/` gains a theme word because of this game.

## 3. Victory

A quest chain with prerequisites, using the engine's objectives-over-facts model and its victory flag.

1. Set a charge on the reactor on deck 4.
2. Set a charge on the reactor on deck 7.
3. Set a charge on the reactor on deck 10, which requires the first two.
4. The third charge opens the core chamber; killing the overseer inside it sets the victory flag.

Each completed charge grants one field upgrade, chosen from three offered, which is the run's non-loot progression.
There is no escape leg.
Backtracking up ten decks without travel or auto-explore would be a slog, and both are deferred engine work.

## 4. Module layout

Delve and Heist each grew a `main.rs` over a thousand lines long, and the engine's own TODO calls that out.
Foundry follows Corsair's shape from its first commit.

```
examples/foundry/src/
  main.rs        plugin wiring and run setup, and nothing else
  content.rs     registry loading and the Names table
  decks.rs       the three band pass-chains and prefab stamping
  droids.rs      the enemy roster: tactics, wits, facets
  gear.rs        weapons, armor, heat and ammunition
  upgrades.rs    field upgrades and the pick-one-of-three
  mission.rs     the quest chain and the victory flag
  input.rs       key handling
  narrate.rs     the phrasebook
assets/
  tiles.ron  items.ron  monsters.ron  prefabs.ron
  abilities.ron  affixes.ron  statuses.ron  quests.ron
```

No file in the game is expected to pass 400 lines.
A file that does is a signal that a concern belongs in its own module.

## 5. Damage

Three kinds, declared in the game's content, since the engine treats damage kinds as content.

| Target | Kinetic | Energy | Ion |
|---|---|---|---|
| Droid chassis | 50% | 100% | 200% |
| Organics: player, hunters, critters | 100% | 75% | 25% |
| Shield droid, shield up | 100% | 0% | 200%, and the shield drops |

The shield droid is the design's teaching moment.
A blaster cannot touch it, a vibroblade goes straight through the ray shield, and an ion round drops the shield outright.
The lesson is taught by a weapon swap rather than by a stat check.

Ion is deliberately low on raw damage, because it answers droids, shields and radar all at once, and section 8.2 gives it a fourth job.

## 6. Weapons

Eighteen weapons, three melee and three ranged for each damage kind, plus two thrown.

Speed is action cost in hundredths of a step, so 100 is one normal turn.
Attack speed requires the engine change in section 10.2.

### 6.1 Three ammunition economies

- Energy runs on heat: unlimited shots against a thermal budget.
- Kinetic runs on ammunition: scarce slugs, no heat, and the only thing that passes a ray shield.
- Ion runs on heat as well, and cycles slowest of the three.

### 6.2 The heat rule

Each shot or swing adds heat to the weapon.
A weapon that reaches 100 locks, and stays locked until its heat reaches zero.
A weapon vents only on a turn its wielder does not fire it, which is what makes heat accumulate on a fast weapon and what makes venting a decision rather than a pause.

Heat lives on the weapon, not on the wielder.
Weapons are entities, so this is a game-side `Heat` component and systems in `TurnSet::React`.
It needs one engine change, section 10.4: the game must learn which weapon a blow came from, and working that out itself would mean re-deriving `Loadout`'s choice.

Dual wielding two heat weapons is therefore a real build: alternate hands, and one vents while the other fires.
It costs the off-hand slot, which is where a shield or a gauntlet ability would otherwise go.

### 6.3 Melee

| Weapon | Kind | Hands | Roll | Speed | Heat | Vent | Burst |
|---|---|---|---|---|---|---|---|
| Vibro-knuckler | Kinetic | 1 | 1d4+1 | 70 | - | - | - |
| Vibroblade | Kinetic | 1 | 1d6+1 | 100 | - | - | - |
| Vibro-axe | Kinetic | 2 | 2d6+1 | 140 | - | - | - |
| Shock gloves | Ion | 1 | 1d4 | 70 | - | - | - |
| Shock baton | Ion | 1 | 1d6 | 100 | - | - | - |
| Electrostaff | Ion | 2 | 1d10 | 120 | - | - | - |
| Plasma cutter | Energy | 1 | 1d6+2 | 90 | 20 | 25 | 5 |
| Phase pike | Energy | 2 | 1d8+2 | 100 | 25 | 25 | 4 |
| Thermal lance | Energy | 2 | 2d6 | 130 | 30 | 30 | 4 |

### 6.4 Ranged

| Weapon | Kind | Hands | Range | Roll | Speed | Heat | Vent | Burst |
|---|---|---|---|---|---|---|---|---|
| Slugthrower pistol | Kinetic | 1 | 6 | 1d8 | 100 | ammunition | - | - |
| Slug rifle | Kinetic | 2 | 12 | 2d6 | 120 | ammunition | - | - |
| Scattergun | Kinetic | 2 | 4 | 3d4 | 130 | ammunition | - | - |
| Ion pistol | Ion | 1 | 5 | 1d6 | 90 | 25 | 20 | 4 |
| Ion carbine | Ion | 2 | 8 | 2d4 | 110 | 35 | 21 | 3 |
| Arc caster | Ion | 2 | 4 | 2d6 | 130 | 45 | 27 | 3 |
| Hand blaster | Energy | 1 | 5 | 1d6 | 80 | 15 | 20 | 7 |
| Blaster carbine | Energy | 2 | 9 | 1d8+1 | 100 | 22 | 22 | 5 |
| Marksman rifle | Energy | 2 | 14 | 2d8 | 140 | 40 | 25 | 3 |

Thrown: a thermal detonator, which deals kinetic damage and sets fire, and an ion grenade, which deals ion damage in a footprint and applies the status in section 8.2.

### 6.5 Where the balance lands

Raw damage per turn sits between 3.5 and 6.5 across all eighteen, so no weapon is dead on arrival.
The separation is in the multipliers and the constraints.

- Ion reads weak and is not, because doubling against droids makes the electrostaff's 4.6 into 9.2 against most of what a foundry contains.
  It is close to useless against hunters and critters, which is the reason to carry a sidearm.
- Kinetic is the shield answer and the only sustained ranged option, paid for in slugs that must be found.
- Energy has the best raw numbers, is resisted by organic armor, and is always on a timer.
- Fast weapons are chip damage and slow ones are commitment: the vibro-axe beats the knuckler per turn but hands initiative to whatever survives the swing.

Two-handed weapons claim the off-hand through the engine's existing slot graph with displacement.
No new equipment machinery is needed for either two-handers or dual wield.

## 7. Armor

Four slots, declared as `SlotDef`s: head, torso, arms, legs.
Twenty pieces, each granting flat armor, per-kind resistance and tags the affix system rolls against.

| Slot | Pieces |
|---|---|
| Head | Scout helmet, commando helmet, rangefinder helmet, blast visor, welding mask |
| Torso | Scrap plate, phase I plate, phase II plate, ablative vest, insulated vest, heavy cuirass |
| Arms | Work gauntlets, combat gauntlets, shock gauntlet, magnetic gauntlets, reinforced bracers |
| Legs | Utility greaves, armored greaves, servo legs, mag boots |

Armor values run 1 to 3 flat, with resistances as the interesting axis: the ablative vest resists kinetic, the phase II plate resists energy, the insulated vest resists ion, and the welding mask resists fire.

The head slot carries the design's payoff.
The **rangefinder helmet** grants the player a `DarkSight` radius, the same component the radar droids carry, which turns an enemy mechanic into a loot goal and makes a run that finds it play differently.

## 8. The roster

Three factions: droids, organics, and the hunter syndicate.
The syndicate is hostile to the player and to the droids, so a hunter meeting a guard droid patrol fights it, which the faction matrix gives for free.

### 8.1 Droids and organics

| Enemy | Decks | HP | Armor | Attack | Notes |
|---|---|---|---|---|---|
| Line droid | 1-9 | 8 | 0 | blaster 1d4, range 5 | The chaff that teaches the systems |
| Probe droid | 1-6 | 6 | 0 | 1d4 energy, range 4 | Flies, radar 4, raises an alarm that wakes the deck |
| Heavy droid | 3-8 | 18 | 2 | wrist blaster 1d6+1, range 6 | Resists kinetic; ion opens it |
| Infiltrator | 4-9 | 14 | 1 | vibroblade 1d6+1, speed 80 | Carries `StealthStats` and hunts in the dark |
| Shield droid | 5-10 | 22 | 1 | twin blasters, 2 x 1d6, range 6 | Ray shield per section 5 |
| Guard droid | 6-10 | 30 | 2 | electrostaff 1d10 ion | Radar 5, sapient wits, picks up better gear |
| Coolant rat | 1-7 | 4 | 0 | bite 1d3 | Flees when hurt |
| Scrap crawler | 2-9 | 7 | 1 | 1d4 kinetic | Mindless, swarms |

Critters are near-immune to ion, which is where an ion-only build learns it needs a sidearm.

### 8.2 Radar, and how it is beaten

Radar droids ignore darkness inside their radius, which breaks a stealth build at the moment it has grown comfortable.
Two counters, both in the engine's existing vocabulary.

- An ion hit applies **sensors down**, a status that strips `DarkSight` for a few turns.
  This is ion's fourth job and the reason its raw damage is low.
- The rangefinder helmet gives the player radar 6, so the dark becomes fightable rather than avoidable.

### 8.3 Bounty hunters

Elite singles, appearing one at a time from deck 4.

| Hunter | HP | Armor | Loadout | Behaviour |
|---|---|---|---|---|
| The slaver | 34 | 3 | Scattergun, vibro-axe | Slow and brutal, drops both weapons |
| The mandalorian | 28 | 2 | Marksman rifle, jetpack blink | Radar 8, fights at range, repositions |
| The assassin unit | 24 | 1 | Dual hand blasters, speed 70 | A droid, so ion melts it |

### 8.4 Mini-bosses

- **Foundry marshal**, deck 3: a heavy droid behind a shield generator stamped as part of its prefab, so the generator must be destroyed before the marshal can be hurt properly.
- **The slaver**, deck 6: the first hunter, in a cargo bay he has already cleared of droids.
- **Magna prime**, deck 9: a guard droid elite with two escorts, radar 6, in the dark.
- **The overseer**, deck 10: a tactical droid core wired into the assembly line, spawning line droids until its feeds are cut. Killing it is the victory condition.

### 8.5 Spawning, and how groups grow

Copied from the sibling project `fantasy-rogue`, where a monster is not one spawn entry but several, each with its own floor range and group size.
The engine's banded tables already carry `(min_band, max_band, weight, min_group, max_group)`.

| Entry | Decks | Group | Weight |
|---|---|---|---|
| Line droid, lone sentries | 1-2 | 1-2 | 100 |
| Line droid, patrol | 2-4 | 2-3 | 90 |
| Line droid, squad | 4-6 | 3-5 | 70 |
| Line droid, formation | 6-9 | 4-6 | 40 |

The falling weight is the important half.
Deck 8 still has large line droid formations in its table, but heavies and guard droids outweigh them, so the chaff thins out through the weights rather than through a rule.

Composed encounters mix types in one draw, mirroring the sibling project's `Group` entry:

- **Patrol**, decks 4-8: one heavy droid and 2-3 line droids.
- **Gunline**, decks 3-6: 3-5 line droids with a probe droid spotting.
- **Sentry post**, decks 8-10: two guard droids in an unlit room.

This needs a `Group` variant in Foundry's monster RON and the loader for it.
It is game-side; the engine's table supplies the banding and the weights.

## 9. The decks

Three bands, each its own pass chain.

| Band | Decks | Chain | Character |
|---|---|---|---|
| Assembly halls | 1-3 | rooms, doors, farthest exit | Lit, orderly, line droids and probes |
| Cast floors | 4-7 | cellular cave, keep largest, scatter | Fire and gas fields, dark in patches, heavies and the first hunter |
| The core | 8-10 | BSP, doors, central start | Tight vaults, mostly unlit, guard and shield droids |

### 9.1 Prefabs, and their variety

`rl-mapgen`'s prefab pass parses ASCII, stamps it, and records marks.
It has no rotation, no flip, no weighted selection and no budget, so the engine change in section 10.3 adds the first three.

On top of that, in the game:

- A **rarity bucket** per prefab, resolving to a placement weight.
- A **per-deck tile budget**, so a deck receives two to four prefabs rather than a fixed count.
- **Mark slots** that either pin a specific spawn, such as the armory's guaranteed weapon, or draw from the deck's band table, such as a random occupant.

One authored vault, stamped in four orientations with a different occupant each time, is most of what "things to discover" means in practice.

### 9.2 What is discovered

Sealed armories with a guaranteed piece behind a locked door, coolant leaks whose gas both hides the player and hurts them, smelter pits that the engine's fire already models, abandoned commando caches holding a dead squadmate's gear and a line of narration, and the three reactor chambers.

## 10. Engine changes

Four, each small, each independently useful, and each landing as its own commit with its own tests before the game depends on it.
10.2 and 10.3 landed on 2026-09-17; 10.4 and 10.5 were found while planning the game and land first in its plan.

### 10.1 Radar needs no engine change

An earlier draft of this design called for a `NoticeStats.dark_sight` field.
Reading the code showed the capability is already there, so the change was dropped.

- `DarkSight(pub i32)` is a component in `crates/rl-bevy/src/lighting.rs:66`.
- `fov::cast` gates every viewer's viewshed through `lighting::gate`, which lets a viewer see an unlit tile within its dark sight, and `update_viewsheds` runs that for every actor with a `Viewshed`, not only the player.
- `stealth::update_awareness` decides `in_view` with `sight.can_see(...)`, which is that gated viewshed.

So a droid carrying `DarkSight(5)` already sees and notices an unlit target within five tiles.
Radar is content: a `DarkSight` component on the droids that should have it.

One nuance is left alone deliberately.
The `lit` flag that `notices` receives is computed from the lighting alone, so an observer seeing a target through dark sight rather than through light gets no `lit_bonus`.
That is the right reading: dark sight shows a shape, and light shows detail.

### 10.2 Attack cost on `MeleeAttack` and `RangedAttack`

Both messages carry a kind, dice and range today, and every blow charges `BASE_ACTION_COST`, so weapon speed cannot be expressed.

This is `docs/TODO.md` section 3, "Attack cost, and a place for a miss".
Tests: a weapon with cost 70 acts more often than one with cost 140 in a headless fight over a seed range; an attack with no cost stated still charges `BASE_ACTION_COST`, so every existing example is unchanged.

### 10.3 Prefab rotation, flip and weighted pick

Rotation by 90, 180 and 270 degrees, horizontal flip, and a weighted pick among a candidate set, with marks transformed alongside the tiles.
The shape is proven in `fantasy-rogue`'s prefab v2, including the rule that marks rotate with the prefab.

Tests: a stamped prefab's marks land on the same tiles after each rotation and flip as the authored layout implies; a weighted pick over a seed range produces the expected distribution; an existing unrotated stamp is byte-identical to today's output.

### 10.4 A blow says what struck it

Found while planning, after the design was agreed.
`DamageEvent` carries the attacker but not the item, so nothing tells a game which weapon fired.
Heat and ammunition both need it, and a game that worked it out itself would be re-deriving `Loadout`'s choice of weapon, which is the failure `docs/PLAN.md` section 1 was written against.
`resolve_attacks` writes a `Struck` message naming the attacker, the target, the worn item the blow came from if any, and whether it was a shot.

Tests: a worn weapon's blow names it; a bare-handed blow names nothing; a shot names the ranged item; with two ranged items worn, the one `Loadout` picks is the one named.

### 10.5 A mind can shoot

Found while planning, after the design was agreed.
No tactic makes a ranged attack: a mind melees, throws or uses an ability, so a droid carrying a blaster would walk up and punch.
This is `docs/TODO.md` section 2, "no keep-at-range for a shooter".
A `ShootAtRange` tactic mirrors `ThrowAtRange`: the snapshot learns how far the actor's own shot carries, and the tactic attacks an enemy two or more tiles off down a clear line.
Keeping at range, backing off to hold a distance, stays in the TODO; shooting when there is a shot is the part the game cannot do without.

Tests in `rl-rules`, without an `App`: a shooter with a clear line to an enemy in reach attacks it; a blocked line, an enemy out of reach, an adjacent enemy and a mind with no shot all decline.

## 11. Progression

Gear and affixes carry the power curve, through the engine's existing affix model with level-scaled grants and depth-banded drops.

On top of that, each completed reactor objective offers three field upgrades, of which the player keeps one.

| Objective | Offered |
|---|---|
| Reactor 1, deck 4 | Combat stims, a healing ability; targeting uplink, +1 range on ranged weapons; reinforced servos, +10% speed |
| Reactor 2, deck 7 | Heat sinks, +8 vent on every heat weapon; slug press, ammunition recovered on a kill; sensor spike, `DarkSight(4)` |
| Reactor 3, deck 10 | Dual processors, a second strike when dual wielding; overcharge, an ability doubling the next shot and filling its heat; deflector plate, +2 armor |

Nine upgrades, three kept per run, which is the run-to-run variety that pure loot does not give.

## 12. Loot

Two sources, both proven in Corsair's content format.

- **On the floor**: a scatter pass places items by depth band, so decks 1-3 offer pistols and scrap plate while the core decks hold marksman rifles and heavy cuirasses. Sealed armories hold a guaranteed good piece.
- **From the dead**: rolled per enemy on death. Line droid 10%, probe droid 10%, heavy droid 20%, infiltrator 25%, shield droid 35%, guard droid 35%, hunters 60%, mini-bosses 100%.

Droids drop parts, power cells and slugs.
Hunters drop the weapon they were using, which is how the player takes an electrostaff off the thing that was hitting them with it.

Every drop roll draws from `Seed::stream`, never from the engine's `CombatRng`.
Corsair gets this wrong in two files today, and it is one of the eleven captured todos, so Foundry is the example that does it correctly from the start.

## 13. The first slice

Playable end to end, and the thing to build first.

- Decks 1-3, the assembly hall chain, with prefabs.
- Three damage kinds and the resistance table.
- Six weapons: vibroblade, vibro-axe, hand blaster, blaster carbine, ion pistol, slugthrower pistol.
- Six armor pieces: one per slot, plus two alternates.
- Line droids with their first two spawn entries, probe droids, heavy droids, coolant rats.
- Reactor one, its objective, and the first field-upgrade pick.
- Ground loot by band and drops on death.
- The engine changes in sections 10.4 and 10.5, each with its own tests; 10.2 and 10.3 have already landed.

What is deliberately absent from the slice: decks 4-10, the other twelve weapons, the other fourteen armor pieces, hunters, mini-bosses, the overseer, composed encounters, and the second and third upgrade picks.

## 14. Testing

The engine's rules apply, and the game is held to them.

- Properties over a seed range wherever a property exists: heat never goes negative and a locked weapon cannot fire; a spawn table has no band gaps; a prefab's marks survive every rotation; drops stay within their stated rates over many rolls.
- Fingerprint tripwires, labelled as such, for a generated deck and for a full scripted run.
- Rules arithmetic tested in `rl-rules` without an `App`, per the engine's panel rule, for anything that is really about the rules.
- Each module in section 4 carries its own tests, so no concern is only covered through `main.rs`.

## 15. Out of scope

| Excluded | Reason |
|---|---|
| An escape leg back up the ten decks | Backtracking without travel or auto-explore is a slog; both are deferred engine work |
| XP and character levels | The engine has no progression module; loot and field upgrades carry the curve instead |
| A pursuit clock spawning enemies behind the player | A system the engine does not have, and not needed by this mission |
| A senses model with jammers and coolant suits | `DarkSight` already serves radar; a full senses model is its own milestone |
| Shipping Star Wars names | Section 2.1 |
| Vehicles, space, or anything above deck 1 | The foundry is the game |

## 16. Open questions

None blocking.
Two worth deciding during planning.

1. Whether the ray shield is a status on the shield droid or a component the game owns, which decides how "ion drops the shield" is expressed.
2. Whether composed encounters land in the slice or after it, since the slice's decks 1-3 use only the gunline.
