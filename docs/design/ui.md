# UI

Status: phases A to G built 2026-09-12; H proposed.
Written 2026-09-11 against `main` at `1264a69`, when `rl-ui` held a message log, a status line and a list menu.
It reads the 17.6k lines across 31 modules of `fantasy-rogue/src/ui/` as the evidence of what a roguelike UI is made of, and decides which third of that belongs to an engine.

## 0. Summary

The reusable half of a roguelike panel is not the drawing.
It is the query into a model.
"Every actor in the player's viewshed, sorted by distance, with a health fraction, a relation and the badges on it" is engine work, and it is the same in every game the engine will ever carry.
"A gold-ruled rail with a small-caps header" is one game's work and must never leave that game.

Three decisions shape everything else:

1. **Every panel splits into a view, a collector and a presenter.**
   The view is the data, the collector is the system that keeps it fresh, the presenter is one way of drawing it.
   A game takes all three, or the first two and draws its own, or none of them.
2. **The engine names no content and no colour.**
   A row carries a `Label` the game set, a glyph the game set, and a `ToneId` interned in a registry.
   Anything the engine cannot know, a wielded weapon or an AI state, arrives as a facet the game pushes after the collector ran.
3. **Opt-in is per panel, not per crate.**
   `EnginePlugins` gains nothing.
   A delve that wants no rail adds no rail.

The engine never says monster, weapon, spell or torch.
It knows actors, items, slots and tones, and a game maps its vocabulary onto them exactly as it already maps tiles and statuses.

## 1. What the split buys

- **The nearby list, for every game at once.**
  Every input already exists: `Viewshed`, `Position`, `Health`, `Faction` with `Factions::relation`, `Afflicted`, and the glyph.
  `rl-rules::balance::threat` already scores danger.
  The view is a query; the reason no game has one yet is that nobody wanted to write it twice.
- **The status line stops being a string each game builds by hand.**
  `StatusLine` is a `String` today, so Corsair rebuilds it every frame in a system over a nine-field `SystemParam`, and the delve and Lamplight each wrote their own `update_status` beside it.
  That is the failure the plan was written against: the engine shipped the data structure and left the behaviour in the game.
- **Equipment rows are nearly free.**
  `rl-rules/src/equip.rs` owns the slot graph, so a view is "walk the slots in declared order, emit filled or empty".
  What is worn and what is not are equally readable, which is the whole content of that panel.
- **Inspect gets its arithmetic tested once.**
  The top third of `fantasy-rogue/src/ui/examine_panel.rs` is pure derivation: hit chance, expected damage, turns to kill, a speed tier, a threat tier.
  That is combat maths sitting in a UI file because there was nowhere else to put it.
  In this engine it belongs in `rl-rules` beside `threat`, under property tests, and the panel becomes a rendering of numbers somebody else proved.
- **Modal input ownership, which every game gets wrong alone.**
  One modal at a time, input gated while one is open, a stack that returns to the parent.
  `fantasy-rogue` solves it with a closed enum and a single-slot return pointer guarded by a `debug_assert` against double-push.
  An open registry of ids and a real stack is both simpler and more general.
- **Badges, bars and hints stop being re-derived.**
  Corsair's `statuses::badges` walks `Afflicted` and folds it to a string.
  Every game with statuses writes that function.

## 2. The model

```rust
/// One entity a panel lists.
pub struct Row {
    /// What it is, for hover and selection.
    pub entity: Entity,
    /// The name the game gave it.
    pub label: String,
    /// Its own glyph and colour, not the theme's.
    pub glyph: Glyph,
    /// Chebyshev tiles from the subject.
    pub distance: i32,
    /// How it stands to the subject.
    pub relation: Relation,
    /// Current and maximum, when it has them.
    pub health: Option<(i32, i32)>,
    /// What the game added that the engine cannot know.
    pub facets: Vec<Facet>,
}

/// A game-authored note on a row: a wielded item, an AI state, a resist chip.
pub struct Facet {
    /// Which kind of note, interned by the game.
    pub key: FacetId,
    /// The words.
    pub text: String,
    /// Its semantic role, which the palette turns into a colour.
    pub tone: ToneId,
}

/// Everything the player can see, rebuilt when the turn advances.
#[derive(Resource, Default)]
pub struct NearbyView {
    /// Actors, nearest first.
    pub actors: Vec<Row>,
    /// Items and props on visible ground, nearest first.
    pub things: Vec<Row>,
}
```

A view holds no `Color`, no rectangle, no layout and no string the game did not supply.
It names `Entity`, so it lives in tier 2 with the rest of the Bevy layer; its derivations do not, and they live in `rl-rules` where they can be tested without an `App`.

The collector runs in `PresentSet::Narrate`, which the reaction-phase commit named for exactly this: what a frame works out once, before the chrome draws it.
The presenter runs in `PresentSet::Chrome`, or `PresentSet::Overlay` for a modal.
A game's facet systems sit between them, ordered after the collector by name.

## 3. Labels, and why not a trait

There is no `Name` component anywhere in the engine.
Corsair resolves a name through `armory.defs.get(kind.0).name`, and the engine must never see that path.

`Label(String)` is an engine component the game writes when it spawns and rewrites when the name changes.
The alternative, a `Describe` trait object the game installs, costs a dynamic dispatch per row, forces a game to implement a wide trait to hand back one string, and is the shape that grows a `Custom { id }` variant six months later.
A component is data, and games already attach a `Glyph` at the same spawn.

The cost is honest and worth stating: a game whose names change at runtime, an unidentified thing that becomes identified, must write the `Label` when it changes.
The engine will not guess, and a stale label is the game's bug to see in its first playtest.

## 4. Tones, not colours

`Theme` is nine fixed `Color` fields and `LogCategory` is a closed five-variant enum.
That is the piece that forces a fork: `fantasy-rogue` already needs a lethal tier distinct from danger, a crit yellow distinct from an accent gold, an XP fill, and a header rule.
A closed enum of five is a closed taxonomy for content, which the house rules already forbid everywhere else.

```rust
/// A semantic role. Interned, so a game adds its own.
pub type ToneId = Id<Tone>;

/// The roles in play, ordered by first use.
#[derive(Resource, Default)]
pub struct Tones(Interner<Tone>);

/// A colour per tone, indexed by the id's dense raw.
#[derive(Resource)]
pub struct Palette(Vec<Color>);
```

The engine interns a small set on startup (`text`, `muted`, `good`, `bad`, `notice`, `frame`, `title`, `select`) so nothing has to be authored to get a working screen.
A game interns `deadly`, sets its colour, and every widget that takes a tone honours it without a line changing in `rl-ui`.
`LogCategory` becomes `ToneId` and `Theme::log_color` becomes an index.

Ids are dense and handed out first come first served, so the palette is a `Vec` and not a map, and iteration order is registration order.
Registration happens in plugin build, which is deterministic, and a game that wants a fixed order registers its tones in one place.

This gives four escapes, each cheaper than the one below it:

0. Retitle and rearrange: a panel takes its rect and its title in its constructor.
1. Swap the palette, including tones the engine never heard of.
   Layout unchanged.
2. Keep the view and the collector, write the presenter.
   Data unchanged.
3. Add neither plugin.

## 5. The backend question

Section 3.12 of the plan says `rl-ui` is Bevy UI ported from `fantasy-rogue/src/ui/`.
What was built draws on the glyph `Terminal`.
Those diverged, and the view split is what makes the divergence stop mattering.

- Terminal only: one typeface, exact-text snapshot tests, a palette swap restyles everything, no layout engine to fight.
  No wrapping, no hover, no sub-cell bars, and `fantasy-rogue`'s node trees are rewritten rather than ported.
- Bevy UI only: layout, wrapping, mouse and tooltips for free, and the existing code ports.
  It forces `bevy_ui` on the delve and on Lamplight, which want a terminal and nothing else.
- Both, over one view layer.
  `rl-ui` ships the views, the collectors and terminal presenters.
  A `node` feature, or `rl-ui-node` if the dependency weight justifies a crate by the rule in section 4 of the plan, ships Bevy UI presenters over the same views.

The third is the decision, because the expensive half is the half that does not care.
A presenter is a few hundred lines against a settled model; the model is the part that took a game to discover.
The `node` half is last in the phases and is built when a game asks for it, not before.

The map view stays a glyph grid in every case.
An ASCII game's map is a glyph grid.

## 6. Where each piece lives

### rl-rules (tier 1, no Bevy)

The derivations, beside `balance::threat`:
hit chance and expected damage from the damage pipeline's own numbers, turns to kill, a speed tier from an integer delay, a bar bucketed to `n` cells, a stat formatted with its sign.
Pure functions over plain numbers, unit-tested without an `App`, and usable by a balance report as readily as by a panel.

### rl-ui (tier 2)

`tone.rs`: `Tones`, `Palette`, the engine's starting set.
`view/`: `NearbyView`, `VitalsView`, `EquipView`, `InspectView`, the `Row` and `Facet` types, and one collector plugin each.
`panel/`: a terminal presenter per view, each its own plugin taking a `Rect` and a title.
`modal.rs`: `ModalId`, the `Modals` stack, `modal_is` and `no_modal` run conditions.
`log.rs` and `menu.rs` stay where they are and move onto tones.

`ChromeLayout` goes.
A single resource holding every panel's rectangle is a god object that grows a field per panel; a panel takes its own rectangle, and a `Dock` helper splits a `Rect` for the games that want the split written once.

### rl-bevy (tier 2)

`Label`, next to `Position` and `Blocks`.
Nothing else: the engine layer gains a component, not a dependency on the UI.

## 7. Modals and input

```rust
/// The modals open, innermost last. Empty means the world has input.
#[derive(Resource, Default)]
pub struct Modals(Vec<ModalId>);
```

`push` opens one over another, `pop` returns to the parent or to the world, `is_open` answers for one id, `top` answers for the drawing order.
`modal_is(id)` gates a modal's own systems and `no_modal()` gates the engine's input, so a game cannot forget the gate and discover its player walking while the inventory is open.
A stack rather than a single-slot pointer because a stack cannot be double-pushed, which is the bug `fantasy-rogue`'s `debug_assert` exists to catch.

The look cursor is the same shape: moving the cursor, cycling to the next visible target and clearing the selection are engine behaviour over the viewshed, what the cursor reports is `InspectView`, and how that is drawn is a presenter.

## 8. What the engine does not get

- Main menu, settings, save slots, key rebinding screens.
  Those are an application's, not a roguelike's.
- Text entry, scrollbars, drag, focus traversal.
  The moment the engine owns a widget toolkit it owns a widget toolkit forever.
- Hover and tooltips, until `rl-render` can map a cursor to a tile.
  That is a real gap and a small one, and it should land in `rl-render` as a mouse-to-tile query before any panel depends on it.
- Key hints from hand-typed strings.
  Hints are worth engine space only if they are generated from a keybind registry, which means the keybind registry comes first or the feature does not come at all.

## 9. Phases

- **A. Tones and labels.**
  `Tones`, `Palette`, the engine's starting set, `Label` in `rl-bevy`, `LogCategory` becomes `ToneId`.
  All three examples look identical afterwards; this phase is the one that changes nothing on screen.
- **B. Nearby, with facets.**
  `NearbyView`, the collector, `NearbyPanel`, and Corsair pushing a facet for what an enemy wields.
  This is the slice that proves the extension mechanism on the case that motivated the doc, and it should not be split.
- **C. Vitals.**
  `VitalsView` and its panel, and `StatusLine` as a hand-built string goes.
  Corsair, the delve and Lamplight each lose a system.
- **D. Modals.**
  The stack, the run conditions, engine input gated on an empty stack, and Corsair's inventory and ledger moved onto it.
- **E. Equipment.**
  `EquipView` over the slot graph, and the sea chest's folded numbers come from it.
- **F. Inspect.**
  The derivations into `rl-rules` with their property tests, `InspectView`, the look cursor, the panel.
- **G. The log grows up.** Built.
  Run-length folding, turn separators, a scrollback modal on the stack from D, filtering by tone.
  The scrollback is a second presenter over `MessageLog` and keeps its own cursor and filter, which is the split proving itself: two screens, one view, neither aware of the other.
  Lines wrap rather than clip, since a screen opened in order to read should not lose the end of a sentence, and the filter cycles only the tones the log holds rather than every role declared.
- **H. Node presenters.**
  Only when a game asks.

## 10. Tests

- A headless `App` with a hand-built world asserts the rows: three actors placed, one out of the viewshed, and the view holds two in distance order.
- A facet pushed by a test system appears on the row it names and on no other.
- A property over a seed range: for any scattered world, every row's distance is non-decreasing and every row is in the subject's viewshed.
- A presenter test asserts exact terminal text, the idiom `rl-ui`'s log test already uses, including the case that a shorter line does not keep the tail of a longer one.
- A palette with a tone the engine never interned renders in the game's colour; the engine's own tones are untouched.
- Input is refused while a modal is on the stack and accepted when it empties.
- The derivations in `rl-rules` are tested there, without Bevy, and the panel tests do not re-assert arithmetic.

## 11. Cost

A collector allocating a `Vec<Row>` of forty rows every frame at the window's refresh rate is waste, and the rows change only when the turn does.
Collectors run on a turn-advanced run condition with a `dirty` flag for the cases that change without a turn, and reuse their `Vec` rather than rebuilding it.
Terminal presenters stay immediate mode and redraw every frame, which costs a memory write per cell and is not worth guarding.

## 11b. What the build changed

Five decisions came out differently, and the reasons are worth keeping.

- **`Name`, not `Label`.** Section 3 argued for an engine component and got the argument right and the name wrong: `Label` collides with `bevy_ui`'s, which every game globbing both preludes would have hit. Bevy's own `Name` is exactly "what to call this entity", so the engine defines nothing and a game that already names entities gets panels for free. The reasoning in section 3 against a `Describe` trait stands unchanged.
- **Views are tier 2, not tier 1.** A row carries an `Entity` for hover and selection, and `Entity` is Bevy. Only the derivations are Bevy-free, and those went to `rl-rules::forecast` where they are tested without an `App`. Section 2 said as much; section 0 overstated it.
- **Every frame, not on a turn boundary.** Section 11 planned a turn-advanced run condition. A frame already rewrites every cell of the map, so a few dozen rows beside it is not the cost worth being wrong about, and a panel that is a turn stale is the kind of bug that survives to a release. The collectors clear and refill their `Vec`s instead.
- **An unset tone is reported at startup, not on use.** Section 4's "warns once" needed interior mutability to mean anything; a check at `OnEnter(Playing)` names every uncoloured tone at once and cannot spam a frame.
- **`ChromeLayout` went and nothing replaced it.** Section 6 said a panel takes its own rectangle, and that was enough: `panel::split_*` hand back both halves of a cut, and a game keeps the pieces in a struct of its own. No layout resource exists.

Found on the way, both fixed: `VitalsViewPlugin` asserted on `StatusRules`, which made a game with no statuses insert an empty registry to have a health bar; and the look cursor would settle on the player when nothing else was in sight, so the inspect panel forecast a duel with yourself.

## 12. Risks

- **The engine ends up owning a widget toolkit.**
  Section 8 is the line, and it is worth re-reading before each phase.
  The test for a panel belonging here is whether it needs engine state to build: nearby needs the viewshed, a settings screen needs nothing.
- **`Label` drifting from the game's registry.**
  The engine will not guess a name, so a game that renames a thing and forgets the component shows a stale row.
  Opt-in per panel limits the blast radius, and a game without panels never writes a `Label` at all.
- **Facets as an escape hatch that swallows the panel.**
  If a game's rows are mostly facets, the view is the wrong shape and the answer is to move the field into the view, not to grow the facet list.
  Two games wanting the same facet key is the signal.
- **A palette a game half-fills.**
  Interning a tone and not colouring it must not be a panic in a shipped game.
  `Palette` returns the `text` colour for an unset tone and warns once, which is visible in a playtest and survives a release.
- **Phase B is the real test of the whole design.**
  If Corsair's weapon facet needs anything the model does not have, the model is wrong, and that is the moment to find out rather than after four more panels are built on it.
