# Undermere, part 2: items and builds

Status: written 2026-10-05, against branch `undermere` at the end of part 1.
The game design is `examples/undermere/DESIGN.md`, section "Items and builds"; this is how that section is built.
The user asked for the design calls to be made without further questions, so every call below is the author's, made to be reviewed afterwards.
The research behind the engine slices was written against part 1's tree and is summarised where each slice cites it.

## Goal

Items are the only progression, and they make builds.
A run's power comes from what the dungeon offered and where the player spent scarce enchantment; every strong thing costs a slot, strength, noise, a charge that will not come back, or a curse; and half of what is found is not yet known.
At the end of this part a run can become a fire mage, a sneaking knife, a plated hammer, a spear skirmisher, a dominator with a pet ogre, or a mess of whatever turned up, without the player choosing any of them up front.

## Engine work

Nine slices.
Each is a loop more than one game writes, and each closes a gap `docs/TODO.md` or `docs/OVERVIEW.md` already names where it does.

### 0. A game's own typeface

The terminal draws every glyph from the system's monospace family: Courier on macOS, which is thin, lacks `✦` and `━`, and differs on every platform, so a game's picture is not its own.
`TerminalPlugin` and `RoguelikePlugins` take an optional typeface, a font file the game ships (`.typeface("fonts/DejaVuSansMono.ttf")` or embedded bytes, whichever Bevy 0.19 loads cleanly on desktop and wasm alike), used in place of the system family wherever it is given.
Undermere ships DejaVu Sans Mono under its own licence in `examples/undermere/assets/fonts/`, so its glyphs are the same everywhere and it may draw `≈` water, `♣` trees, `Ω` the gate and the box-drawing and block shades it wants.

### 1. A use aimed at somewhere

Today a `use` trigger lands on the user where they stand (`crates/rl-bevy/src/items.rs`, `land_triggers`), and only throwing and firing aim an item at all.
A staff or a wand is zapped from the pack at a target, so a `use` trigger may carry an aim, in the vocabulary abilities already use: a `TargetMode` shape, a range, and an `Aim` saying what it wants under it.
The bag's use key on an aimed thing writes `AimUse { user, item }`, which opens the targeting cursor as `AimThrow` does; the cursor previews through the same `Bystanders::land` an ability's does; confirm writes the use with its cell; `land_triggers` lands the trigger's effects on the footprint with the item's `EffectBonus`; the flight is cued with the trigger's look; charges are spent as now.
An item still never lends an ability (`docs/design/items.md`): the zap is the staff's own trigger.

### 2. A use on another thing

A scroll of enchanting is read onto an item of the player's choosing.
A `use` trigger may instead ask for one of the user's own things, with an optional tag the thing must carry: the bag's use key opens the bag again in a choosing mode under the trigger's prompt ("Enchant which?"), confirm writes `UseOn { item, on }`, and the landing carries `on` to its effects, which a game's own effects read.
Nothing is spent when the choice is put away.

### 3. Things not yet known

An item carrying `Unknown` is shown by its name and its slot and nothing else: the bag, the inspect panel and the targeting banner leave out what its triggers do and what it is worth, and say "unknown" in its place, so identification is a game's choice of name and a marker, not a redaction every game writes in `ViewSet::Annotate`.
Its real triggers still work: an unknown potion drunk still does what it does.

### 4. Things that will not come off

An item carrying `Stuck` refuses `Unequip` and refuses to be displaced by something put on over it (`Equipment::equip`'s displacement, and `EquipFromGround`'s), each a refusal with a reason in words, and the bag stops offering its take-off key.
What unsticks it is the game's: its own effect removes the component.

### 5. Blows and shots with a shape, and blows on the unaware

`MeleeAttack` and `RangedAttack` take an optional shape: `Sweep`, every foe adjacent to the attacker, and `Pierce(n)`, the target and the next `n` cells beyond it in line, each struck by its own roll.
`Struck` is written once per swing, so heat and ammunition count swings, and a `hit` trigger lands on each foe struck.
A blow on a target that has not noticed its attacker, by stealth's `Aware`, is multiplied: `CombatRules::unaware(percent)` for everyone, and a weapon's own `MeleeAttack::unaware(percent)` above it, so a dagger's five times outranks the house's three.
This closes `docs/TODO.md`'s "A shot has no shape".

### 6. A status that takes its holder's turns

`StatusDef` gains `holds`: while a status that holds is on an actor, each of its turns is spent as a wait before its mind or its player is asked, as `Work` claims a worker's.
Paralysis, a web and sleep are each one line of a game's statuses.

### 7. Speed from a stat

A rule names a stat whose value is a percent added to an actor's `Speed` wherever the turn loop reads it, so haste and slowness are statuses that modify a stat, and part 1's dropped `chilled` comes back.

### 8. Ammunition

Foundry's `ammo.rs` is a loop a second game with a bow writes again, so it moves into the engine: a weapon names the tag of what one shot spends, a shot spends one from the wielder's bag, a weapon with none is dry and `Loadout` stops finding its shot, and the gear and bag rows say so.
Foundry moves onto it.

## The game

### Identification

Potions, scrolls, staffs, wands and rings wear a face per run: potions a colour ("murky", "crimson", "violet", "amber", "black", "pink", "turquoise", "smoky", "milky", "silver"), scrolls a title of nonsense syllables, staffs a wood, wands a metal or a bone, rings a stone.
`Lore`, saved with the run, holds each kind's face and whether it is known, drawn from the run's seed; a thing of an unknown kind spawns `Unknown` under its face's name.
Using one makes its kind known when what it did was plain to see, and every thing of that kind is renamed at once, with a line in the log ("That was a potion of strength.").
A scroll of identify makes one chosen thing known.
Weapons, armor and rings are known by kind and not by level: their level reads `+?` until they have been worn for 200 turns or identified.

### Enchanting

A scroll of enchanting is the commonest thing in the dungeon after gold, and at least one lies on every floor.
Read onto an item it raises the item's level by one: armor one more, a weapon's damage one more, a staff's power and charges one more and its recharge faster, a ring's power one more, and a stuck item whose level reaches zero comes unstuck.
The armory re-derives an item's components from its definition and its level whenever the level changes, through the same function that made it, so a level is never applied twice.

### Stats, strength and accuracy

Undermere registers its first stats: strength, accuracy, evasion, stealth, regeneration and light.
The adventurer's strength is 12.
Heavy weapons and armor name a strength; for each point the wearer is short, a weapon costs 4 accuracy and armor 1 armor, kept current whenever what is worn or the wearer's strength changes.
A potion of strength adds one for good.
The run's hit model becomes the engine's `Percent`: accuracy 100 for everyone less evasion, so misses happen and strength matters; nothing evades in this part but a shield's wearer.

### The armory

| Kind | Members, with the trade each makes |
| --- | --- |
| Weapons | dagger (quick, five times on the unaware), sword, broadsword (two hands, strength 14), axe (sweeps), war axe (two hands, sweeps, strength 15), mace (slow, heavy, strength 16), war hammer (two hands, slower, heavier, strength 20), spear (pierces one behind), war pike (two hands, pierces one behind, strength 18), bow (spends arrows), darts and javelins (thrown and picked up again) |
| Armor | leather 1, scale 2 (str 13), chain 3 (str 13), banded 4 (str 15), splint 5 (str 17), plate 6 (str 19); the heavier, the louder each step |
| Off hand | shield: armor 1 and evasion 10, and it takes the hand a two-hander needs |
| Staffs | firebolt (a bolt that burns), lightning (a beam through every foe in line), poison (a bolt that poisons for long), slowness, blinking (a step of several cells to a chosen spot), conjuration (spectral blades that fight for you and fade) |
| Wands | domination (a foe becomes yours, surer the more it is hurt), teleport other, polymorph (a foe becomes something else from anywhere in the dungeon), beckoning (pulls a foe to you), plenty (a foe becomes two; on an ally, a gift) |
| Rings | regeneration, stealth, light, wisdom (staffs recharge faster), reaping (blows recharge staffs), awareness (find what is hidden) |
| Potions | healing, extra healing, strength, life (more health for good), haste, fire immunity, darkness (the dark hides you), descent (sink to the floor below), and three better thrown: incineration, paralysis (a cloud that holds whoever breathes it), confusion |
| Scrolls | enchanting, identify, teleportation, remove curse, recharging, magic mapping, discord (foes in sight turn on each other), and two that hurt: aggravate monsters, summon monsters |

A staff's level is its power and its charges; a wand has a few charges that never come back unless a scroll of recharging refills them.
Rings, armor and weapons may be found at a negative level, and then they are stuck, and sometimes worth wearing anyway.
A potion of darkness brings back what part 1's dim ambient took from the dark: a pool of true darkness round the drinker, where dark sight matters and the unlit go unseen.

### What a floor holds

Gold as now, and a budget of items per floor drawn by kind: potions and scrolls most, weapons and armor less, staffs, wands and rings least, each kind's members weighted by depth, a scroll of enchanting guaranteed, and a deeper floor's items more often enchanted.
About fifteen scrolls of enchanting turn up over a run.

### Effects of the game's own

Enchant, Identify, RemoveCurse, Recharge, Blink, Teleport, TeleportOther, Polymorph, Beckon, Dominate, Plenty, Conjure, Discord, Aggravate, Summon, MagicMapping, Descend and Darkness, each registered with `add_effect` and described in its own words, so the bag says what a known thing does.

## Testing

Each engine slice has its own tests in its crate.
The game's include: a scroll of enchanting read onto chain mail makes it one better, and never twice; an unknown potion drunk becomes known and renames its kind everywhere; a cursed ring cannot be taken off until enchanted to zero or uncursed; strength short of a war hammer's costs accuracy, and a potion of strength gives it back; an axe sweeps every foe beside the adventurer and a spear pierces the one behind; a dagger kills an unaware rat in one blow; a staff of firebolt zapped at a goblin burns it and spends a charge that comes back; a wand of domination makes a beaten goblin follow the adventurer; every floor holds a scroll of enchanting, over a span of seeds; every glyph the game draws is one its typeface has.

## Not in this part

Charms, artifacts (part 3's mini-bosses carry them), monsters using what they carry (part 3), shops (part 5).
