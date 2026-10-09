# Undermere, part 5: the town

Status: written 2026-10-08, while part 3 is being built on branch `undermere` and part 4 on `undermere-dungeon`; docs only, with no plan yet, since a plan is written against the code as it stands when the part begins.
Revised the same day to the user's direction: the townsfolk are named by part 6's name generator, a shop's goods are rolled, and the temple's rites are priced verbs, not items.
The game design is `examples/undermere/DESIGN.md`, section "The town"; this is how that section is built, and the content sheet it is built from.
The user asked for the design calls to be made without further questions, so every call below is the author's, made to be reviewed afterwards.

## Goal

Gold is found below and spent in Mereholt, so a run has a second economy beside the scrolls of enchanting: what to buy, what to sell, and when to come back up for it.
Mereholt is a place with people in it.
Five keepers each run a shop or a service, six townsfolk each have something to say, and some of what they say is true of this run's dungeon and of no other.
No two runs meet the same people or the same shelves: everyone in Mereholt is named by part 6's name generator each run, and a shop's goods are rolled, their levels, their runes and, at the curio shop, what they even are.
The question the skeleton left open is answered: the gate takes the adventurer back mid-run, at a price, and returns them to where they left.

## The gate goes back

The adventurer can come back to Mereholt mid-run in two ways, and goes back down to where they left.

- **Walking out.** Floor one's way up is the gate's far side, as the skeleton built it, so climbing every stair comes out on the island. Nothing changes here.
- **A waystone.** A carried stone, used from the pack, that sets the adventurer kneeling for ten turns (the engine's `Work`, kind "calling the gate"). Any harm breaks it off and the stone is kept; finished, the stone is spent, the adventurer is warped to the town side of the gate, and the cell they knelt on keeps a **gate mark**, a prop glowing the gate's violet.
- **Going back down.** The gate in Mereholt leads to the last gate mark, at its cell, or to floor one's way in when there is none. A new call moves the mark; there is only ever one.

The adventurer walks out of Mereholt with one waystone, the temple sells more at 40 gold, and the deeper floors hold a rare one among their things.
A waystone cannot be used in Mereholt or on the island, and says why.
Part 6's mercenaries come along on the terms they follow through a stair, within three cells and in sight, so the plan makes the waystone's warp say `Travelled` as a way through does.

Why this shape:

- **Not a one-way trip.** Without a return, gold found below is only score, the shops matter once, before floor one, and DESIGN's "gold is found below and spent here" is not true.
- **Not walking only.** The floors are kept whole and do not refill, so climbing nine cleared floors to shop is a long walk with nothing in it, and a player who would rather not walk would simply never shop.
- **Not an escape.** Angband's word of recall is a delayed teleport that works mid-fight, so it is also the best escape in the game. Ten turns of kneeling that any harm breaks off is a trip home from a quiet room and never from a fight, and the stone's price is the cost of the trip.
- **No skipping.** The gate leads back to the mark, not to the deepest floor, so a trip home never moves the adventurer forward.

## Engine work

Two slices, one of them a few lines; the rest is content on seams the engine already has, and on part 6's `rl-rules::namegen`.

### Trade between two actors

A keeper is an actor offering `trade`; trading is a loop every game with a shop writes, and it touches the pack, stacks and identification, so it is the engine's.

- **What is for sale** is what the seller carries: its `Inventory` is its stock, so a shop's stock is saved with the run the way any actor's pack is, and restocking is the game filling it.
- **A price** is the game's, kept current on each item as a `Worth` component, since only the game knows what a level, a kind or a curse is worth. A thing with no `Worth` is not for sale and cannot be sold.
- **The currency** is items carrying a tag the game names, counted by their stacks, so gold stays a pile of coins in the pack that can be dropped, stolen and counted for the score as it is today.
- **The seller** carries a `Trader` naming the tags it buys and the share of a thing's worth it pays, in hundredths.
- **Buying and selling** are resolved at once, take no turn, and are refused with a reason a screen can show: not enough gold, a kind this seller does not buy, a thing with no worth. The trade itself is the turn the `trade` offer cost to take up; a shop is a screen, not a place, and nothing that could act on the time is in it.
- **The screen** is `rl-ui`'s, a view, a collector and a presenter as every panel is: the seller's stock with prices on one side, the buyer's pack with what the seller would pay on the other, the buyer's gold at the foot, and a line the game supplies under the seller's name.

### An offering with a price

An `Offering` gains an optional price in the trade slice's currency.
An offer whose taker carries less is refused, as one needing a tag they lack is, and says how much it wants, so the offers screen greys the row and shows the price; taken up, the price is spent before the game's `Interacted` answer runs.
The temple's rites are this, and so is any toll, ferry or shrine a later game writes; a service is a verb with a price, not a thing that pretends to be carried for an instant.

## What is already there

Read from the code on 2026-10-08, so the plan can lean on it:

- Part 3's actor offers: an actor carrying `Offers` offers its verbs to whoever stands beside it, under the same gate and refusal as a prop's, and bumping a non-hostile actor that offers one thing takes it up; `talk` and `free` are already exercised in `rl-bevy`'s bump tests. A keeper offers `trade`, a townsperson `talk`.
- `Work` with `Works::begin` for an actor that is not a mind, broken off by harm, which the captive's `free` already uses for the adventurer.
- A way through is a `Transition` component the game can rewrite, with `Arrive::At(cell)`, and `WarpRequest` moves anyone to any place: the gate mark needs no engine change.
- A prefab's `Mark` is a position the game finds by its glyph, which is how the keepers' and townsfolk's posts are placed.
- A worn thing's `Lends` is folded per kind, once, at load (`wearing.rs`); a rune makes it per thing, folded when the thing is made and saved with it, which is the one change the trait model needs.
- Part 6's spec puts `rl-rules::namegen` in the engine with `NameSet::draw_distinct`, and its `names.ron`; this part adds a `townsfolk` set and needs part 6's engine slice built first, or built here if part 5 lands first.
- The tally counts gold as it is taken up below, so the kit's purse and gold from selling are never counted as treasure.

## The game

### Peace in Mereholt

Nothing is harmed in Mereholt.
Undermere's damage stages, which part 3 gave an armor stage, gain one that drops any harm landing on the town's map, so a stray firebolt neither kills a keeper nor sets the tavern alight, and no rule has to ask whether the townsfolk fight.
The keepers and townsfolk are on a `townsfolk` faction at peace with the adventurer, the one part 6's candidates stand on, so a bump talks or trades rather than strikes.

### Gold

The adventurer walks out with 50 gold, enough for darts or a potion before floor one and not enough for both.
A floor lays 0 to 2 piles of 3d8 times its number, and kobolds, goblins, archers and ogres carry the same at 40 or 80 in a hundred; that comes to roughly 30 gold a floor times its number, about 90 by floor two, 450 by floor five and 1350 by floor nine.
The prices below are set against that estimate; part 5's first task measures the real figure with the soak, and the prices move with it before anything else is tuned.

A thing is priced at what the adventurer knows of it.
A level known above nought adds 100 a level; a level not yet known counts as nought, as the vitals and the forecast already count it, so a shop never tells a level.
A thing whose kind is not known, or that shows a curse's sign, has no worth: the keepers will not buy what nobody can name, and nobody buys a curse.
A keeper pays a quarter of a thing's worth, rounded down.
Artifacts are never sold; the curio buys them at 400.
A known rune adds 150 to a thing's worth, and a known bad rune leaves it no worth at all, as a curse does.

### The shops and their prices

Keepers name what they sell, but a shelf is not an identification service: a ware is listed by its true name in the shop's own colour, never its face, so browsing says which kinds exist and not which bottle in the pack is which.
Buying one puts it in the pack wearing its face, and that makes its kind known.
Stock is drawn when the run starts and redrawn when the adventurer comes back to Mereholt having reached a deeper tier than the stock was drawn for: tier one is floors 1-2, then 3-4, 5-6, 7-8, and 9-10.
So a run sees at most five stockings, and going home twice from the same depth finds the same shelves less what was bought.
Each stocking draws from the run's own `Seed::stream("wares", stocking)`, so the same seed meets the same shelves.

### What a shop rolls

`assets/wares.ron` is shaped as part 6's `mercenaries.ron` is: per keeper, `parts` of rows each drawn as a `LootTable` draws at a band, with `chance`, `weight`, `group` for a stack's count and `bands` for the tiers a row appears in, and one shared `levels` table rolled as a `LevelTable`, so a hireling's kit and a shop's shelf are read by the same code and tuned in the same shape.
On top of the draw, three things make a stocking unlike the last:

- **Levels.** Anything that takes levels rolls one from `levels` at the tier: nought mostly, +1 from tier 2, +2 from tier 4. A keeper never stocks below nought.
- **Runes.** A weapon or a suit of armor may carry a rune, below; a keeper stocks only good runes, named, from tier 2, about one row a stocking.
- **The curio's unknowns.** One curio row a stocking is a potion, scroll, staff, wand or ring of a kind not yet known, drawn as a floor draws, sold by its face at a flat 80 gold: "I couldn't tell you what it is, and I've stopped guessing." It is the one exception to keepers naming what they sell, and the one way to buy a gamble, a potion of life or a scroll of aggravate monsters alike.

### Runes

A rune is a trait in `traits.ron`, lent by one thing rather than by its kind: the thing's own `Lends` is its kind's folded with its rune's, made when the thing is made, so a rune is written once and reads as the rest of the trait model does.
A weapon's rune lands on its `hit`, through the thing's own triggers; an armor's rune lends what a ring does, a status, a resistance, sight in the dark.
Runes are rolled wherever a weapon or armor is made, not only in shops: on the floors from floor 3 at one thing in ten, a third of those bad, and in part 6's hireling kits through the same maker, so part 6 needs no change to carry them.
A rune found below is not known: the thing is named "runed sword" until the rune shows itself in the adventurer's sight, the thing is worn long enough to be known, or it is named at the temple or by a scroll of identify; a shop's rune is named on the shelf.

| Rune | On | What it does | Good |
| --- | --- | --- | --- |
| of venom | weapon | a blow poisons, more turns a level | yes |
| of embers | weapon | a blow sets alight one time in four | yes |
| of frost | weapon | a blow chills and slows | yes |
| of confusion | weapon | a blow confuses one time in four | yes |
| of paralysis | weapon | a blow paralyses one time in eight | yes |
| of mercy | weapon | a blow mends what it strikes one time in three | no |
| of plenty | weapon | a blow makes two of what it strikes one time in twenty | no |
| of the salamander | armor | fireproof | yes |
| of mending | armor | regenerating | yes |
| of shadows | armor | stealthy, as the ring of stealth | yes |
| of sight | armor | sees in the dark | yes |
| of clamour | armor | every step as loud as plate | no |


**The smithy**, in the stone building with the forge: six rows drawn, plus arrows always.
It buys weapons, armor and things to throw.

| Ware | Price | From tier |
| --- | --- | --- |
| arrows, 10 | 20 | 1, always |
| darts, 8 | 24 | 1 |
| javelins, 4 | 32 | 1 |
| dagger | 30 | 1 |
| sword, spear | 60 | 1 |
| mace | 70 | 1 |
| axe, bow | 80 | 1 |
| leather armor, helm | 40 | 1 |
| shield | 60 | 1 |
| scale mail | 100 | 1 |
| broadsword | 120 | 2 |
| war pike | 130 | 2 |
| war hammer | 140 | 2 |
| war axe | 150 | 2 |
| chain mail | 160 | 2 |
| banded mail | 240 | 3 |
| splint mail | 320 | 3 |
| plate armor | 450 | 4 |

**The apothecary**, in the west building with the shelves: potions of healing always, then four rows drawn.
It buys potions and scrolls.

| Ware | Price | From tier |
| --- | --- | --- |
| potion of healing, 3 | 40 each | 1, always |
| scroll of identify | 40 | 1 |
| potion of darkness, of confusion | 30 | 1 |
| potion of incineration, of paralysis | 40 | 1 |
| scroll of remove curse | 40 | 1 |
| potion of descent, of fire immunity | 50 | 1 |
| scroll of teleportation | 50 | 1 |
| potion of haste | 60 | 2 |
| scroll of magic mapping | 60 | 2 |
| scroll of discord | 40 | 2 |
| potion of extra healing | 90 | 2 |
| scroll of recharging | 80 | 3 |

Never stocked anywhere: scrolls of aggravate monsters and of summon monsters, which keepers buy at 10 and 15 like any known thing, so a known bad scroll is worth carrying up.

**The curio shop**, in the south-east building with the counter: four rows drawn, rare and dear, and one unknown.
It buys anything with a worth.

| Ware | Price | From tier |
| --- | --- | --- |
| wand of plenty | 100 | 1 |
| wand of beckoning | 120 | 1 |
| wand of teleport other, of polymorph | 150 | 2 |
| staff of slowness | 180 | 1 |
| staff of poison | 200 | 1 |
| staff of blinking, of conjuration | 220 | 2 |
| staff of firebolt, of lightning | 250 | 2 |
| ring of light, of stealth, of wisdom, of awareness | 250 | 2 |
| ring of regeneration, of reaping | 300 | 3 |
| wand of domination | 300 | 3 |
| scroll of enchanting | 200 | 2, at most one a stocking |
| potion of strength | 300 | 3, at most one a stocking |
| potion of life | 800 | 5, a quarter of stockings |

A scroll of enchanting is the most valuable thing in the game and is laid on every floor, so the curio sells at most four in a run, each at about two floors' gold.

**The temple of the Still Water**, in the east building with the altar and pews: three rites, each an offering with a price, priced the same all run and never out of stock, and waystones by trade.
The priest offers `trade`, `mend`, `unbind` and `name`, so a bump opens the offers screen with the price on each row.
A rite is a verb, not an item: the game answers it in `Interacted` with the effects it lands on the one who took it up, from `assets/rites.ron`, a row per verb with its price and its effects, written in the effect words `items.ron` uses.

| Rite | Price | What it does |
| --- | --- | --- |
| `mend` | 30 | health made whole, and every status that lasts until cured cured |
| `unbind` | 60 | everything worn free to come off, as a scroll of remove curse |
| `name` | 60 | everything worn known: its level, its curse and its rune |

| Ware | Price | |
| --- | --- | --- |
| waystone | 40 | see "The gate goes back" |

Naming takes everything worn rather than one thing chosen, so a rite asks nothing after it is taken up and its price is paid once, before it lands; the scroll of identify stays the way to name a potion, and the temple is the way to read gear.

### The people of Mereholt

Each person is a role, a row of `assets/townsfolk.ron`: the role's words, glyph, colour, a post (a mark in `mereholt.ron`), whether they keep to it or wander the square and lanes, what they offer, and their lines.
Their names are drawn when the run starts, from the `townsfolk` set in part 6's `names.ron` through `NameSet::draw_distinct`, against every name the run has given, so no townsperson shares a hireling's name and Mereholt's smith is somebody else next run.
The names are saved with the run, as part 6 saves the names it has given.
The look panel gives the role as a facet under the name, `Hesk`, "the smith".
A line may name another person by role, `{gatewarden}`, and says that person's name this run.
A line may carry `when`, one of `deepest: n` or `slain: "name"`, and is said only once it holds.
Talking takes a turn and writes the next line not yet said, in the speaker's colour, as `Name: "line"`; once every line that holds has been said, the last is said again.
A shopkeeper offers `trade` and nothing else, so a bump opens the shop, and their greeting is the line under their name on the trade screen.
Several lines teach a rule the game never explains anywhere else; that is what they are for.

**The keepers.**

- **The innkeeper**, the Sunken Lantern's, at the counter. Offers `talk`, which says this run's rumours (below); part 6's candidates wait at the tables.
- **The smith**, at the anvil: "Steel, leather, and arrows by the bundle. I buy what's straight and sell what's sharp."
- **The apothecary**, behind the counter: "Everything on my shelves has a name, and I'll tell you it. What you bring me had better have one too."
- **The curio dealer**: "Rare things, dear things. Things the dead had no more use for."
- **The priest**, at the altar: "The Still Water mends, unbinds and names. It asks a little gold for the candles."

**The townsfolk.**

- **The gatewarden**, at the head of the causeway, keeps the post.
  "The stones let anyone through. It's the coming back they're particular about."
  "Carry a waystone. Kneel where it's quiet, and the gate will find you."
  `deepest: 5` "You've been deeper than my brother got. He's still down there, I'd guess."
- **The lamplighter** walks from lamp post to lamp post.
  "Every lamp in Mereholt is lit by dusk. The lake doesn't like the dark either."
  "The deeper you go, the less your lantern helps. It isn't the oil. It's what's down there."
- **The drunk**, by the tavern door, keeps the post.
  "Went down once. Came up. Never again."
  "Daggers. The sleeping ones never see a dagger coming."
- **The gravedigger**, in the gardens.
  "I bury what comes back up. Mostly it's just the name."
  "Fire or a hammer, for the bones that stand back up. Nothing else keeps them down."
- **The fishwife**, on the shore.
  "Nets came up empty again. Fish know better than to swim over the gate."
  "Heavy armor is loud. Half the lake can hear a man in plate."
- **The child** wanders the square.
  "Are you going down? Will you bring me a bone? A real one?"
  `deepest: 3` "You came back! Was it dark? Was it scary?"
  `slain: "Grubnak"` "Everyone says you killed the goblin king! Was he big?"

### Rumours

The innkeeper says one rumour a talk, in a fixed order: the floor ten boss, then each mini-boss the run drew, then each band whose biome was drawn from more than one; then "That's all I've heard. Come back when you've been somewhere."
Rumours are always true and never exact: they say what is down there, never which floor or how to beat it.
They read the run's draws, which part 3 makes for the boss and the mini-bosses and part 4 for the biomes; a draw not fixed by the time the run starts has no rumour, and the plan checks that each one is.
The lines are `assets/rumours.ron`, one per drawable thing.

| Drawn | Rumour |
| --- | --- |
| the Drowned King | "Nets come up with crowns in them some nights. Something down there still thinks it rules." |
| the Mother Below | "The well's tasted of eggs since spring. {gatewarden} says the lake has a mother." |
| the Ember Wyrm | "The lake steams over the deep water now, winter and all. Nothing cold breathes like that." |
| Grubnak | "Goblins sing marching songs below, and they've got themselves a king." |
| the Widow | "A delver came back up wrapped in silk, and none of it his." |
| the Bone Choir | "There's singing near the crypts, and nobody down there has a throat left." |
| the Drowned Abbess | "The old abbey sank with its abbess at prayer. Some say she hasn't stopped." |
| the Forgemaster | "Somebody's working a forge under the lake. You can hear the hammer from the jetty." |
| the Upper Halls | "Just past the gate it's the old king's halls. Broken stone and long corridors." |
| the Cellars | "Just past the gate it's cellars this season. Casks, damp, and rats the size of dogs." |
| the Fungal Deeps | "Past the first stairs it's caves lit by mushrooms. Don't breathe what the big ones puff." |
| the Spider Warrens | "Past the first stairs it's webs, whole galleries of them. Burn what you can't cut." |
| the Catacombs | "Halfway down you walk through the dead. They were buried. It didn't take." |
| the Sunken Library | "There's a library halfway down, half full of water. Scholars went in. Pages came out." |
| the Drowned Cathedral | "Deep down there's a cathedral. Mind the bridges; the water's colder than it looks." |
| the Underlake | "Deep down there's a lake under the lake, and things in it that never saw the sun." |

### The map

`mereholt.ron` gains a mark for each role's post and nothing else: the innkeeper beside the tavern's counter, the smith by the anvil, the apothecary and the curio dealer on the shop side of their counters, the priest beside the altar, the gatewarden at the causeway's head, the drunk at the tavern door, the fishwife on the shore and the gravedigger in the gardens; the lamplighter and the child start at a lamp post and on the square.
A keeper stands where a bump from the shop floor reaches them, since an offer is taken up from beside its actor, not across a counter.
The houses stay empty; a house with nobody in it is a quieter town than one with a name in every bed.

### What the rail shows

The vitals gain the adventurer's gold as a facet, so what a trip home could buy is always on the screen.
A gate mark is a landmark the adventurer remembers once seen, so the map shows where the gate will bring them back.

## Testing

The trade slice in `rl-bevy` and `rl-ui`: a buy moves the ware and its price in currency and takes no turn; a sell pays the seller's share; each refusal is refused and says why; a shop's stock survives a save; an offering with a price is refused to a taker who carries less, and spends the price when taken up.
The game's: a waystone's kneeling is broken by a blow and the stone kept, and finished it brings the adventurer to the gate and leaves a mark the gate then leads to, through a save; no waystone works in Mereholt; nothing is harmed in Mereholt; a thing of unknown level sells at its nought price over a span of seeds, and an unknown kind or a cursed sign does not sell; buying a potion makes its kind known; a rite lands its effects and spends its price, and is refused when the gold is short; the townsfolk's names are drawn distinct from each other and from every hireling's over a span of seeds, and survive a save; a line's `{role}` says that role's name; a shop never stocks below nought or a bad rune, over a span of seeds; a rune found below is named only once it shows itself or the thing is known; a weapon's rune lands on its blow; the curio's unknown row is of a kind not yet known when it is stocked; stock is redrawn on a return from a deeper tier and not from the same one; each line's `when` holds it back until it holds; every rumour's subject is a draw the run has fixed when it starts; the kit's purse and gold from selling are not counted as treasure.
The soak gains the gold found by each floor, over its fifty seeds, and the prices are checked against it.

## Not in this part

Hiring mercenaries at the Sunken Lantern, which is part 6's spec, with candidates at the tavern's tables for 20 gold and half what these shops would ask for their kit; a bank, a home or a stash; haggling; stealing from a shop; keepers who restock between tiers; the houses' insides.
