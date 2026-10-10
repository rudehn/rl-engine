# Undermere, part 6: mercenaries

Status: written 2026-10-08, during part 3 on branch `undermere`, ahead of parts 4 and 5; revised the same day to the user's direction: a name generator in the engine, one kind of mercenary with a rolled kit, and no tempers.
Revised again on 2026-10-09, when part 5 had merged: the name generator is built (`rl_rules::namegen`, on `main`), part 5's town is what this part stands on, and the party panel is the one engine slice left.
The game design is `examples/undermere/DESIGN.md`, section "Mercenaries"; this is how that section is built.
The user asked for the design calls to be made without further questions, so every call below not named as the user's is the author's, made to be reviewed afterwards.

## Goal

The adventurer walks out of Mereholt with whoever they can pay.
At the Sunken Lantern a few candidates wait each visit, each with a name of its own and a kit rolled from a data file: a weapon, armor, perhaps something to drink or throw.
The roll is the character: one candidate is a cheap hand with a rusty blade and nothing else, the next a costly one in good mail with a sword two levels up and a potion besides.
The adventurer may lead two at once.
A mercenary follows, fights what the party fights, runs when badly hurt, drinks its own potions, wears gear better than its own, and goes down every stair beside the adventurer.
One key switches the party between follow, hold and fight freely.
A mercenary dies for good, and the end screen remembers it by name.

## What it stands on

Part 3 built most of the engine half, so this part is mostly game content over seams that exist:

- **Actors that offer verbs.** `Offers` on an actor, answered as a prop's offer is, with an `Interacted` the game reads; never offered to one the actor is hostile to. A candidate offers `hire`; a hired mercenary offers `give` and `dismiss`.
- **Following one leader.** `Leader(Entity)` on the follower and `Keep::leader` in its brain, saved by `rl-save` with the actor, falling back to its allies while the leader is gone.
- **A mind uses what it carries.** `Wits::USES` and the `UseCarried` tactic: it drinks what mends once hurt and points what it holds at a foe, through the resolver the player's uses go through.
- **A mind wears what it finds.** `Wits::EQUIPS` and the fetch tactic: it walks to gear lying in sight whose `gain` is above nought and puts it on.
- **Allies.** `allies.rs` already turns a monster to the adventurer's side, by its `Faction`, its kind's ally mind and the `Ally` marker the save keeps.
- **Travel.** `WarpRequest` with `Arrive::At(cell)` puts an actor on another map at a chosen cell, and `Travelled` says when the adventurer went through.
- **Loot.** `LootTable` draws a weighted row among those that apply at a band, and `LevelTable` rolls an enchant level by band; the kit roll is both, over rows of its own.

It also stands on part 5, `2026-10-08-undermere-town-design.md`: the Sunken Lantern on the town map, the `townsfolk` faction at peace with the adventurer, gold as a pile of coins spent by tag, a `Worth` on every item for the shops, an `Offering` with a price, refused to a taker carrying less and paid before the game's `Interacted` runs, and a gate that goes back mid-run by a waystone whose warp says `Travelled` as a way through does.
Part 5 built all of that, and more this part reads: `townsfolk.rs`'s `NameSets` (the loaded `names.ron`, whose `townsfolk` set this part's `mercenary` set joins) and `GivenNames` (every name the run has given, saved, which candidates draw against); the innkeeper at the tavern's counter, whose tables the candidates stand at; the vitals' gold facet; and `Tally::fallen`.

## Engine work

Two slices, each a thing every game with people in it writes; the first is built.

### 1. Names (built)

Built on its own branch and merged to `main` ahead of part 5, as `rl_rules::namegen`; what follows is what it is and why.

The user's call, with libtcod's name generator as the reference rather than the template.

libtcod's `namegen` reads a config file of named sets, each with syllable pools by position (`syllablesStart`, `syllablesMiddle`, `syllablesEnd`, `syllablesPre`, `syllablesPost`), single letters (`phonemesVocals`, `phonemesConsonants`), an `illegal` list of substrings, and `rules` in a string language: `$s$v$35m$e` is a start syllable, a vowel, a middle syllable 35 times in a hundred, and an end, with `%50` in front of a rule giving its chance of being chosen and `_` standing for a space.
A drawn name is thrown away and drawn again when it holds an illegal substring, a letter three times running, a pair repeated back to back (`arnarn`), or a run of three letters seen again later in it.

What `rl-rules::namegen` keeps: named sets in one file, pools of syllables, patterns that pick among them with chances, and rejection with a redraw.
What it changes, and why:

- **Patterns are RON, not a string language.** A part is a pool, a pool with a chance, or literal text, written as variants that `Names`-style loading checks once: a pattern naming a pool the set does not have fails at load with both names, where `$x` in libtcod is a character it copies through.
- **Pools are named by the set, not fixed by position.** Start, middle and end are a convention of one kind of name; a set of bynames (`"the Lame"`, `"Halfhand"`) is a pool like any other, and a pattern of given name, space and byname is three parts.
- **Patterns are weighted, not retried.** libtcod picks a rule, rolls its `%`, and picks again on a miss; a weight draws once, and only the ratio matters, as the loot table's does.
- **Case is the set's.** `case: Title` capitalises every word, `Upper`, `Lower` and `AsWritten` the rest, so the same generator writes `Brenna Halfhand` and an unknown scroll's `XOMBA ULTH`.
- **The rejections are libtcod's, plus a length.** Three of a letter running, a pair repeated back to back, a run of three letters repeated, a forbidden substring from the set's `forbid`, a doubled or edge space, and a name outside the set's `length: (fewest, most)` letters.
- **Drawing is bounded and distinct.** `NameSet::draw(&mut impl Rng)` redraws up to a fixed number of attempts and returns `None` past them, never looping; `NameSet::draw_distinct(&mut impl Rng, taken: impl Fn(&str) -> bool)` also rejects a name the caller already has, which is how a run never names two hirelings alike. Load checks the shape, not the luck: every pool named exists and is non-empty, every weight is above nought, and every pattern can make a name inside the length bounds; whether a set's rejections leave enough names is a property test over a range of seeds in the game that ships the set, since engine code never makes a generator of its own.

`NameBook` is a file of sets keyed by name, loaded with `namegen::load`, and `NameBook::set("mercenary")` answers one or a load-time error naming the missing set.
It is tier 1, Bevy-free and WASM-clean, and draws from whatever stream the caller hands in.

### 2. The party on the rail

Nothing on the rail says how a companion is doing once it is out of sight, and the nearby panel lists only what is in it.
A `PartyView` lists the actors whose `Leader` is the player: each one's name, glyph, health and the game's facets, in the order they joined.
A collector refills it in `ViewSet::Collect` from `Leader`, `Health` and `Name`, wherever the follower stands, so one left behind on another floor still has a row, marked as elsewhere.
A `PartyPanel` presents it in the rectangle it is given, one line a follower with a health bar in the vitals' own tones, and draws nothing with nobody following.
A game pushes a follower's own words, such as its order, as `Facet`s in `ViewSet::Annotate`.

## The game

### One kind

There is one kind, the mercenary, a row of `monsters.ron` with the `mercenary` trait in `traits.ron`, which gives `["sapient", "uses", "equips"]`, the adventurer's footing in health and a flee threshold of a quarter.
What makes one unlike the next is its kit and its name, not its kind.
Kinds of mercenary, a bowman or a cleric with a brain and an ability of its own, are left for later, and the content model makes them a row each when they come.

### The kit roll

`mercenaries.ron` says what a candidate may carry, with a top-of-file comment listing every field:

```ron
(
    fee: (base: 20, share_pct: 50),
    parts: [
        (name: "weapon", chance: 100, rows: [
            (item: "dagger", bands: (0, 10), weight: 3),
            (item: "sword", bands: (0, 10), weight: 4),
            (item: "spear", bands: (0, 10), weight: 2),
            (item: "broadsword", bands: (3, 10), weight: 1),
        ]),
        (name: "armor", chance: 80, rows: [
            (item: "leather armor", bands: (0, 10), weight: 5),
            (item: "scale mail", bands: (0, 10), weight: 2),
            (item: "chain mail", bands: (3, 10), weight: 1),
        ]),
        (name: "consumable", chance: 50, rows: [
            (item: "potion of healing", bands: (0, 10), weight: 4),
            (item: "dart", bands: (0, 10), weight: 2, group: (4, 8)),
            (item: "javelin", bands: (0, 10), weight: 1, group: (2, 3)),
        ]),
    ],
    levels: [
        (bands: (0, 10), level: -1, weight: 2),
        (bands: (0, 10), level: 0, weight: 6),
        (bands: (0, 10), level: 1, weight: 2),
        (bands: (3, 10), level: 2, weight: 1),
    ],
)
```

The names above are a first cut against `items.ron`, and the numbers part 7 tunes.
A candidate rolls each part on its own: whether it has one by `chance`, then which by the part's rows as a `LootTable` draws at a band, then a level for anything enchantable by the `levels` as a `LevelTable` rolls at the same band.
The band is the deepest floor the run has reached, nought on the first evening, so a later visit can find a broadsword and a level two that the first could not.
A part with no row at the band is simply not carried.
A thing rolled below nought is cursed as one found below is, so a weak candidate may be weak for good once it has put the thing on; the look panel says so, as it says of the adventurer's own.

Each part is a list of rows a game may grow, and a part is a name the file gives it, so a fourth part, a ring one time in ten, is a row of `parts` and no code.

### The fee

The fee is the kit's worth: `base`, plus `share_pct` of the `Worth` part 5 keeps on everything the candidate carries at its rolled levels.
A stacked candidate costs what its gear would, at a discount, and a weak one costs little, so the roll is a choice between them rather than a lottery.

### Candidates

Each visit to town draws three candidates from the run's own `Seed::stream("tavern", visit)`, stood at the tavern's tables, on the townsfolk faction part 5 introduces.
Each is named by `NameBook::set("mercenary")` through `draw_distinct`, against every name the run has given, so no two hirelings of a run share one.
Undermere's `names.ron` holds the `mercenary` set: given names from start and end syllables with a middle one time in three, and one name in four followed by a byname from a pool of its own, `Brenna`, `Tolvar`, `Osk Halfhand`.

The look panel says what is known of a candidate as facets: its kit, each thing named as the adventurer would know it, and its fee.
A weapon's and armor's level is told, since a hireling says what it is selling; a potion the adventurer has not identified is named by its face.
A candidate offers `hire`.
`hire` is part 5's offering with a price, the candidate's fee, so the engine refuses it to an adventurer carrying less and pays it before the game hires.
A candidate also offers `talk`, a line saying what it carries and what it asks, so a bump opens the offers screen with the price on the `hire` row rather than spending gold on a step.
The game refuses `hire`, with the reason in the log and nothing paid, when two mercenaries are already following; that check is the game's, so it is made before the price is taken, by taking the offer off every candidate while two follow and putting it back when one leaves.

### Hired

Hiring puts the mercenary on the adventurer's side by `allies.rs`, gives it `Leader(adventurer)` and a game `Mercenary` component holding its name, fee and order, and swaps its `hire` offer for `give` and `dismiss`.
Its name is a proper noun in the log, with no article: "Brenna strikes the goblin."

It thinks with one of three brains, each built once at load, beside the ally mind:

- **Follow**, the order it is hired on: flee when hurt, use what it carries, strike what is beside it, throw, keep within three of the leader, hunt, then fetch gear. Hunting below keeping station means it fights what comes within its leash and never more than three cells from the adventurer.
- **Hold**: flee when hurt, use what it carries, strike, throw, keep its post. It is given `Post` at the cell it stood on when ordered, and neither hunts nor fetches.
- **Fight freely**: flee when hurt, use what it carries, strike, throw, hunt, then keep within three of the leader and fetch. It chases anything it sees and comes back when nothing is left.

### The order key

`o`, under "Act", "order the party: follow, hold, fight freely", cycles the order of every hired mercenary at once, writes one log line naming the new order, and costs no turn.
The party panel shows each mercenary's order as a facet, and so does the look panel.
Ordering one mercenary apart from the other is left out: with at most two, one key is the whole of what the design asks for.

### What it carries, and what it knows

A mercenary's own kit is its own: it knows what it was hired with, and drinks its own potion when hurt whether or not the adventurer knows the kind.
A potion drunk in the adventurer's sight is learned exactly as if the adventurer had drunk it, so a hireling with a murky potion is also a way to find out what murky potions are.
Anything else, given or picked up, it uses only once the adventurer knows it.
In `DecideSet::Perceive`, after the engine has filled its snapshot, a game system drops from `carried` anything not yet known that is not marked `OwnKit`, as `throw_what_strikes` already trims a thrower's missiles; and it recounts the `gain` of gear lying in sight that is not yet known at its kind's plain numbers, the same correction part 2 makes for the vitals and the forecast.
Without this a mercenary drinking a found potion when hurt would say it mends, and one stepping past a sword would say it was cursed.
A mercenary that puts on something cursed cannot take it off, as the adventurer cannot.

`give`, on a hired mercenary, opens the pack's own chooser; the chosen thing moves to the mercenary's pack for a turn, and it puts it on on its own turn when it is gear worth more than what it wears.
Nothing is taken back: what a mercenary was given comes back only when it dies or is dismissed and drops it.
The cost of leading one is the loot it takes up first, which the hold order prevents.

### Travelling

A mercenary does not carry `Wits::TRAVELS`.
The engine's travel takes a travelling mind through any way it runs to, and a hurt mercenary fleeing alone down a stair would be lost to the run for a reason the player never chose.
Instead, on the adventurer's own `Travelled`, every hired mercenary on the map they left that is alive, not holding, and within four cells with the adventurer in sight is sent after them by a `WarpRequest` to a free cell beside where they landed, nearest first, then by entity.
Four, one past the leash of three, because a follower reads four cells off mid-walk round a diagonal (Ruling 5), and the commonest way to take a stair is to walk onto it and take it at once (Ruling 12).
One that cannot be placed, with every cell beside the landing taken, stays and says so in the log.
Every other one left behind gets a line too, saying why: too far behind, out of sight of where the adventurer went, or holding.
One left behind keeps its `Leader`, waits frozen on its floor as everything on a map nobody reads does, and follows again when the adventurer comes back for it.
This is the same for the stairs and for the Drowned Gate.

### Leaving

`dismiss` is an offer that asks first, the engine's `Offering::asking`: the offers screen asks it, No picked out, and No costs nothing.
A yes takes it up for a turn and takes the mercenary out of the party: back on the townsfolk faction, with `Ally`, `Leader` and its order taken off.
It keeps its own kit, worn and carried, and sets down where it stands whatever it was given or picked up.
`hire` is offered again at the fee of the kit it keeps, so hiring one and letting it go never buys its kit for less than a candidate would ask for it.

### Death

A mercenary dies for good and drops what it carries, as any actor does.
The log says it in the tone the game uses for the adventurer's own losses.
The tally counts `mercenaries_lost` and keeps a fallen list of name, the floor and what killed it.
The end screen lists the fallen under the stats, and the records keep the count, the line `DESIGN.md` already plans.

A kill landed by a hired mercenary counts on the Slain line, where the tally now credits only the adventurer: the gold spent on the party is the adventurer's choice, and a party that kills is the return on it.
Nothing else in the score changes.

### Saving

`Leader`, `Post` and the actor are the engine's to save; `Ally` is already the game's.
The game saves `Mercenary` and `OwnKit`, and puts back on load what is not saved: the brain for its order and its `give` and `dismiss` offers.
A candidate saves its name and fee, and is given back `hire`, so a run saved in the tavern finds the same three at the tables.
The names a run has given are saved with the run, for `draw_distinct` to keep reading.

## Content files

`mercenaries.ron` holds the fee, the kit's parts with their rows, and the levels, as above.
`names.ron` holds the name sets, the `mercenary` set first; a later part may move the unknown scrolls' titles onto a set of their own.
The kind is a row of `monsters.ron`, and `mercenary` a row of `traits.ron`.
Each file starts with a comment listing every field it may hold.

## Testing

The engine's, in `rl-rules`, over a range of seeds:
a drawn name passes every rejection and sits inside the length bounds;
a pattern's parts appear at their chances within a tolerance;
`draw_distinct` never returns a taken name, and returns `None` rather than looping when every name is taken;
a set naming a missing pool, a nought weight, or a pattern that can never fit the length fails at load saying which;
the same stream draws the same name, and case is applied per word.
The party view's collector in `rl-ui`: a follower on another map has a row marked elsewhere, a non-follower has none, and rows keep the order they joined in.

The game's, each named as the property it checks:
Undermere's `mercenary` set draws a hundred distinct names on every seed of a range, so a long run never runs dry;
the tavern draws three candidates a visit, each with a weapon, armor at about four in five and a consumable at about one in two over a range of seeds, and a later visit's band reaching rows the first evening's cannot;
a candidate's fee is its base plus its share of its kit's worth;
hiring spends the fee and is refused when the gold is short or two follow;
a follower keeps within three of the adventurer across a floor with nothing to fight;
on hold it stays at its post while a foe is in sight beyond reach;
it drinks its own potion when hurt, unknown or not, and the adventurer learns it when in sight, and never drinks a found potion the adventurer has not identified;
it puts on a better sword lying in sight, and passes over an unknown one counted at plain numbers;
it comes down the stair beside the adventurer from four cells away, and is left behind from five, or on hold, and the log says so;
its death is counted, named on the end screen, and survives a save and load;
a hired party survives a save and load with its leader, orders, offers and own kit.

## Not in this part

Kinds of mercenary and tempers, by the user's direction; per-mercenary orders; mercenaries gaining anything but gear; taking a thing back from one; companions met below other than freed captives, which part 3 builds; the tavern's rumours, which are part 5's.
