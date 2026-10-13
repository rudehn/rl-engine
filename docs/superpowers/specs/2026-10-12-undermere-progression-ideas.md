# Undermere: backgrounds, experience and talents (ideas)

Status: ideas only, nothing decided, no code.
Written 2026-10-12 to explore classes, experience and a skill tree in Undermere.

## Where this starts from

Pillar 3 of `examples/undermere/DESIGN.md` says items are the only progression: no experience, no levels.
Everything below breaks that pillar on purpose, so the first question is how hard to break it, and the last section offers a version that bends it instead.

What already exists, and makes most of this cheap:

- **The trait is Undermere's unit of "what a thing is".**
  A monster lists traits, a worn item `lends` traits, and a trait can carry speed, sight, dark sight, resists, armor, light, notice, abilities, statuses held for life and triggers on hit, struck or died.
  A talent can simply be a trait the adventurer holds for the run, applied the way `wearing.rs` applies what a worn thing lends.
- **The engine's ability system is complete and unused by the player.**
  `AbilitiesPlugin` runs RON abilities with shapes, costs, requirements, cooldowns and effects, `Grants` and `Known` decide who knows what, and `rl-ui` ships `AbilityMenu`, `AbilityPanel` and `AbilityKeys`.
  `DESIGN.md` records "the adventurer with no abilities of their own" as deferred.
- **Fourteen monster abilities are ready to reuse**: blink, ward ally, mend ally, firebolt, slowness, conjure blades, war cry, spit web, raise dead and others.
  A first set of player abilities needs almost no new effect code.
- **Danger-weighted kills are already counted** for the score, and `bestiary_lore` already records when a kind's trick is learned.

Gaps this would hit:

- Traits have no plain stat bonuses (accuracy, evasion, strength, stealth), so a talent like "+1 accuracy" needs a `stats` field on traits.
- Undermere registers no pools (`abilities.ron` says so), so abilities run on cooldowns only until a pool is added.
- The engine has no experience or level subsystem at all.

## Backgrounds (classes)

A background is a row in a new `backgrounds.ron`: a name, the words the title screen says about it, a starting kit, the traits held from the first turn, the abilities known from the first turn, and the talent pool it draws from.
Because the parts are traits and abilities, a new background is data, not code.

When it is picked:

- **At New Game, from all of them.** The simplest option, and the one players expect.
- **At New Game, from three drawn for the run.** This fits pillar 5 ("no two runs show everything") and stops a favourite becoming the only one played.
- **In Mereholt, at a trainer.** The adventurer starts as nobody and pays gold at the tavern, the temple or a guild hall for a background on the first evening.
  This ties the background to the town, but it competes with the starting gold that shops already want.

First-cut backgrounds, each tied to something Undermere already does well:

| Background | Kit | Starts with | Tree leans on |
|---|---|---|---|
| Sellsword | sword, chain mail | shield bash | blows, armor, war cry, holding a door |
| Delver | dagger, leather, more torches | dark sight 2, quieter steps | stealth, traps, noise, a thrown knife |
| Lamplighter | staff, a brighter lantern | flare (a burst of light that dazzles) | light: pillar 1 as a playstyle, shrouding, burning |
| Acolyte | mace, holy symbol | mend ally | wards, cleansing, mercenaries kept alive |
| Beastkin | spear, hide | a wildlife companion | wildlife, allies, perception |

The Lamplighter is the background that only this game could have, and it is worth building first if only one is built.
The Acolyte is the one that makes mercenaries matter more, because mend ally and ward ally already exist as monster abilities.

## Experience: where it comes from

This is the decision with the most knock-on effects, because whatever gives experience is what the player will start doing more of.

1. **Kills, weighted by danger.**
   The classic choice, and the numbers already exist.
   It fights the stealth, noise and avoid-it-if-you-can design, rewards grinding if anything respawns or wanders in, and makes mercenaries steal the player's experience unless kills by allies count too.
2. **Depth and discovery.**
   Experience for reaching a floor for the first time, a vault opened, a mini-boss slain, a captive freed.
   It cannot be ground, the total over a run is known, so balancing is easy, and sneaking past a troll is never a loss.
   The risk is that levels feel like a clock rather than a reward.
3. **Knowledge.**
   Experience for the first time a kind of monster is killed or its trick is seen (`bestiary_lore` already notices this), a potion or scroll kind is identified, or a writing is read.
   This makes pillar 4 ("readable, with secrets") the engine of growth: the adventurer gets stronger by understanding the dungeon.
   Like discovery, it is bounded and cannot be ground.

Recommendation: discovery plus knowledge, with first kills of a kind counting as knowledge.
It keeps every playstyle honest and gives a run of roughly ten to twelve levels, about one per floor plus a few earned early.

What a level gives:

- One talent pick, always.
- Little or no maximum health, so that "the adventurer at the bottom is the adventurer at the top with better things" mostly survives, and potions and armor still decide whether you live.

## The tree

Three shapes, from most familiar to most roguelike:

1. **Fixed tree per background.**
   Three branches of about five talents each, the deeper ones needing the shallower.
   Easy to read and plan, but after a few runs there is one best path and every Sellsword is the same Sellsword.
2. **One shared web, with the background as the starting point.**
   Path of Exile in miniature.
   The most expressive, but the hardest to show on an ASCII screen and to balance, and the most work to author.
3. **A dealt tree.**
   At each level, three talents are offered, drawn from the background's pool and a shared pool, each talent may require another, and one is taken while the other two are lost.
   It is a tree, because the prerequisites still shape it, but it is dealt rather than browsed, so no two runs build the same adventurer.

Recommendation: the dealt tree.
It fits pillars 3 and 5 (the two you pass on are what the choice costs, and the pool is bigger than one run sees), it is one modal on the modal stack rather than a tree screen, and a talent pool is a list in RON.
A small number of capstones, offered only at levels eight and up, give each branch something to aim for.

Talent kinds, most of which are already a trait:

- **Passive**: armor +1, fire resistance, dark sight, perception, a quieter notice, regenerating while resting.
- **Reactive**: a trigger on struck (riposte), on hit (bleeding, a chance to slow) or on died (a last burst).
- **Active**: an ability from `abilities.ron`, used through the engine's ability menu and keys.
- **Upgrades**: a talent that replaces an ability the adventurer has with a stronger row (blink to blink farther), so tiers are data and not code.
- **Bargains**, for pillar 3's "every gain costs something": strong with a price, such as +25% damage while below half health but no reading scrolls in a fight, or dark sight 4 but a dimmer lantern.

## Abilities need fuel

Options, in order of how much they add:

1. **Cooldowns only.** Already supported, nothing to add, and enough for a first try.
2. **One shared pool**, called focus or nerve, refilled by resting or by a new floor.
   The engine's `Cost::Pool` already spends a registered stat; whether the engine refills a pool over time needs checking.
3. **A pool per background**, such as oil for the Lamplighter and faith for the Acolyte.
   This is the most flavour and the most UI, and probably too much for an experiment.

Recommendation: cooldowns first, and a single pool only if playtests show abilities being spammed or ignored.

## How it meets the rest of the game

- **Enchanting** is the item currency, and talents are a second one.
  Watch that the two do not make each other pointless; one tie between them is a talent that makes enchanting stronger.
- **Mercenaries** could have a background too, cheaply, since they are already "one kind plus a kit".
  Letting them level is a second experiment, not part of the first.
- **Score and records**: the background and the level reached go on the end screen and in the records, as lines.
- **Monster balance**: the bestiary was tuned against an adventurer whose only growth is items.
  Talents that are mostly sideways (utility, reach, reaction) rather than raw damage keep that tuning mostly valid.
- **Saving**: experience, level and talents taken must be saved; the abilities' cooldowns already are, through the engine.
- **The screen**: an experience line on the vitals panel, a level-up modal for the pick, a character screen listing background and talents, and the engine's ability panel for actives.

## Engine or game

The engine's rule is "own the loop or leave it out".

- **Experience and levels** are generic enough for the engine: an integer, thresholds as data, a level-up message and a view field on vitals.
  Foundry could use the same thing for Cogmind-style growth by depth.
- **The dealt pick** ("offer N from a weighted pool, honouring prerequisites, seeded from its own stream") is also generic and testable without an `App` in `rl-rules`.
- **Backgrounds, the talent content and where experience comes from** stay in Undermere.

A first experiment can live entirely in Undermere and move the generic parts into the engine only once they have earned it.

## A gentler variant that keeps pillar 3

If experience turns out to feel wrong, the same talent machinery works with no experience at all:

- **A scroll of enchanting read on yourself** gives a talent pick instead of a level on an item.
  Talents compete directly with gear for the game's one currency, so every pick costs something, and pillar 3 still holds.
- **Shrines** found on floors offer the three-card pick in place of a level-up.

Backgrounds work under either variant, because a background is just a different start.

## Order to try it in

Each step is playable alone, and each answers a question before the next is built:

1. **The adventurer uses abilities.** Wire the engine's ability panel and keys for the player and grant one ability from a starting item.
   Is using an ability fun in this game at all?
2. **Backgrounds**, with no experience: kit, traits and starting abilities, picked at New Game.
   Do classes alone give runs enough variety?
3. **Experience and the dealt pick**, from discovery and knowledge, behind a run option so items-only stays playable beside it.
   Does growth make the game better or just easier?
4. **Capstones, bargains and upgrade chains.**
5. **Mercenary backgrounds**, if step 3 earned its place.

## Questions to settle before any of it is built

- Does pillar 3 break, bend (the enchanting variant) or stay?
- Is the background picked at New Game, dealt three per run, or bought in town?
- Does experience come from kills, from discovery and knowledge, or from both?
- Fixed tree, shared web or dealt picks?
- Does maximum health grow with level?
- Do allies' kills or discoveries count toward the adventurer's experience?
