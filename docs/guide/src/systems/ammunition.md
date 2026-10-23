<!-- documents:
     plugins: AmmoPlugin
     files: crates/atglyph-bevy/src/ammo.rs
            crates/atglyph-bevy/src/combat.rs
            crates/atglyph-bevy/src/items.rs
            crates/atglyph-ui/src/view/gear.rs
     fingerprint: 5569a737 -->

# Ammunition

Ammunition is what a shot spends: a weapon names a tag, and every shot it fires takes one thing of that tag out of its wielder's bag.
With none left the weapon is dry, its shot put by until the bag holds the tag again, and whoever wears it fights with whatever else it has.
Whether a weapon is dry is never kept in step with the events that touch a bag; it is read off the bag itself every pass, so no way of emptying or filling one can leave a weapon wrong.

## Turning it on

`AmmoPlugin` is opt-in, since most of a game's weapons never run on ammunition, and a game with none pays nothing for the idea.
It registers `AmmoChanged`, configures `AmmoSet::Resolve` inside `TurnSet::React`, and runs `spend_ammo` then `sync_ammo` in it, chained, so a shot's unit comes off its bag before every bag is read for what it can still feed.
Its `finish` declares `depends_on::<CombatPlugin>`, for the `Struck` a shot writes, and `depends_on::<ItemsPlugin>`, for the bag both systems read.
A game that wants its own reaction to a shot or to a bag settled first orders against `AmmoSet::Resolve`, never after either function.

## The model

`Ammo { tag }` on an item makes it fed: the tag, registered as every tag is, is what one shot spends, and a weapon carries it beside its `RangedAttack`.
What it spends is any carried thing with that tag in its `Tagged` and a `Stack`, so a quiver is a stack of arrows and its count is what is left.
`spend_ammo` reads every ranged `Struck` whose item is fed and takes one off the first stack in the attacker's bag with the tag and something left in it, despawning the stack at nought; a blow, a `Struck` naming no item and an attacker with no bag are passed over.
`sync_ammo` then reads every fed item in every `Inventory`, worn or not, and sets it dry or loaded from that bag alone: with no unit of its tag left it takes the `RangedAttack` off, puts it by as `Stowed::Ranged` and marks the item `Dry`, and with one back it puts the attack back and takes both off.
Only a change writes `AmmoChanged { item, dry }`, so a game that tells its player a weapon has run dry or loaded again reads that rather than watching components come and go, as Foundry's narration does.
`Stowed` is combat's holder for an attack a worn thing cannot make right now, and a game benching a weapon for a reason of its own, heat among them, puts it in the same holder; `Dry` marks the ones ammunition put there, so `sync_ammo` restores only what it put by and a game's own reason, querying its own cause, never sees one ammunition benched.
`Loadout` therefore finds no shot on a dry weapon and falls through to the next worn thing with an attack, knowing nothing of ammunition, and `Loadout::dry` is the one place that answers by name whether a worn thing is dry.
`GearViewPlugin` notes a worn fed weapon's rounds on its gear row as a facet keyed `ammo`, `N left` counted fresh from the player's bag or `dry` in `Tones::BAD`, with `note_ammo` after `collect_gear` in `ViewSet::Collect`, so the rail says whether the next shot is there without the pack being opened.
Nothing of it is saved, since it is the bag's: a continued run reads each weapon dry or loaded from the bag it restored.

## Using it

A fed weapon is a row naming the tag one shot spends, and what it spends is a stack carrying that tag, which Undermere's bow and its arrows are.

<!-- include: ../../../../examples/undermere/assets/items.ron:bow -->
```ron
    (name: "bow", glyph: '}', color: (0.65, 0.5, 0.32), price: 80, wear: (slot: "main hand", also: ["off hand"]), tags: ["weapon"], ammo: "arrow", strength: 12,
     ranged: (range: 10, effective: 7, roll: "2d4+1", kind: "pierce", look: (glyph: '|', color: (r: 191, g: 173, b: 128)))),
    // The bow outshoots anything thrown: it reaches ten cells and is true
    // to seven, and strikes for 2d4+1 where a javelin, the best thrown, is
    // 1d6+1 to six and true to four. Arrows are ammunition and nothing
    // else, spent a shot at a time and never thrown; darts and javelins,
    // made to be thrown and picked up again where they land, are what
    // someone without a bow has.
    (name: "arrow", glyph: '|', color: (0.75, 0.68, 0.5), price: 2, tags: ["arrow"], stack: true),
```

## The line

The engine decides what a shot spends, when a weapon is dry, and that a dry weapon has no shot to offer a player, a mind or a forecast.
The game decides which weapons are fed and by what tag, how many a stack holds, and what is said when one runs dry or loads again, in its own words over `AmmoChanged`.
A weapon is dry for want of its tag in its wielder's bag and for nothing else: there is no magazine and no count kept on the weapon, so two weapons on one tag share one supply, and a game that wants reloading writes it as the stack a reload makes.
Ammunition was Foundry's own until a second game had a bow, and the loop it wrote, spending on a shot and drying on an empty bag, was the loop every game with a gun would have copied, so the loop moved into the engine and Foundry kept its content and its words.
What a row says of a dry weapon is a facet rather than a field of `Row`, because `Row` is the shape vitals and nearby share too, and a magazine's count is a fact about a fed weapon and not about every row a panel shows.

## Where it lives

`atglyph-bevy` is tier 2 and holds the economy in its own module, beside combat rather than inside it, since a game of cutlasses adds `CombatPlugin` and never this.
`Stowed` and `Loadout::dry` are combat's, so a weapon benched for heat and one benched for want of arrows fall through the same way, and the plugin's own tests prove that a bag emptied by any means dries its weapon, with a bag filled by hand and no turn queue at all.
`atglyph-ui` reads `Ammo` off the gear view's own entities, since it depends on `atglyph-bevy` and not the other way round, and a game drawing its own rail from `GearView` reads the same facet.
