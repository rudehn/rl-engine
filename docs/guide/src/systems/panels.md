<!-- documents:
     plugins: UiPlugin, AbilityPanel, AbilityViewPlugin, GearPanel, GearViewPlugin,
              InspectPanel, InspectViewPlugin, LogPanel, NearbyPanel, NearbyViewPlugin,
              OffersPanel, OffersViewPlugin, ScrollbackPanel, SheetPanel, SheetViewPlugin,
              TargetPanel, TargetViewPlugin, VitalsPanel, VitalsViewPlugin
     files: crates/rl-ui/src/lib.rs
            crates/rl-ui/src/tone.rs
            crates/rl-ui/src/facet.rs
            crates/rl-ui/src/focus.rs
            crates/rl-ui/src/modal.rs
            crates/rl-ui/src/controls.rs
            crates/rl-ui/src/menu.rs
            crates/rl-ui/src/log.rs
            crates/rl-ui/src/view/mod.rs
            crates/rl-ui/src/view/vitals.rs
            crates/rl-ui/src/view/gear.rs
            crates/rl-ui/src/view/nearby.rs
            crates/rl-ui/src/view/inspect.rs
            crates/rl-ui/src/view/ability.rs
            crates/rl-ui/src/view/target.rs
            crates/rl-ui/src/view/offers.rs
            crates/rl-ui/src/view/sheet.rs
            crates/rl-ui/src/panel/mod.rs
            crates/rl-ui/src/panel/vitals.rs
            crates/rl-ui/src/panel/gear.rs
            crates/rl-ui/src/panel/nearby.rs
            crates/rl-ui/src/panel/inspect.rs
            crates/rl-ui/src/panel/ability.rs
            crates/rl-ui/src/panel/target.rs
            crates/rl-ui/src/panel/offers.rs
            crates/rl-ui/src/panel/sheet.rs
            crates/rl-ui/src/panel/log.rs
            crates/rl-ui/src/panel/scrollback.rs
            crates/rl-rules/src/forecast.rs
            crates/rl-bevy/src/plugin.rs
            crates/rl-render/src/terminal.rs
            crates/rl-render/src/map_view.rs
     fingerprint: 3a6487c0 -->

# Panels

A panel is three things and never one: a view, a collector and a presenter.
The view is a resource of plain data, the collector is the system that refills it each frame, and the presenter is one way of drawing it.
The query is the half worth sharing, because "every actor in the viewshed, nearest first, with a health fraction and a relation" is the same sentence in every roguelike and a gold-ruled rail with small-caps headings is one game's taste.
So a game takes all three, or the first two and draws its own, or neither.

## Turning it on

`UiPlugin` is the base every other plugin here needs, and it draws nothing: it holds `Tones`, the `Palette`, `Facets`, `Modals`, the direction and cursor bindings and the repeat pace, the `Focus` and the `Sighted` list, the `Controls` registry, and the `MessageLog`.
The log lives here rather than with the panels that draw it because a game writes to it from its own systems whether or not anything draws it, so a headless test adds this plugin and has a log with no panel in sight.
It chains `ViewSet::Sight`, `Collect`, `Annotate` and `Speak` inside `PresentSet::Narrate`, the frame's work-out-what-to-say phase, and `finish` declares `depends_on::<CorePlugin>`.
Each panel after that is its own plugin, constructed with the `Rect` it draws in and whatever titles and hints it carries, and it adds its view plugin behind it when the game did not.
A strip draws in `PresentSet::Chrome` and a screen in `PresentSet::Overlay`, which is what makes a modal cover the thing it is about.
`VitalsPanel`, `NearbyPanel`, `GearPanel` and `LogPanel` are strips; `InspectPanel`, `AbilityPanel`, `SheetPanel`, `OffersPanel`, `ScrollbackPanel` and `TargetPanel` are screens, and every screen has a modal.
Which of the two layers declares it is not the same for all six, and the rule is that it goes wherever the behaviour is: the two cursors are the view's, so `InspectViewPlugin` and `TargetViewPlugin` declare the modal and the key themselves and a game that takes the view alone still gets a cursor it can open, while `AbilityPanel`, `SheetPanel`, `ScrollbackPanel` and `OffersPanel` declare theirs in the presenter.
A key that opens a screen is declared in `finish`, after the game's own so a controls screen lists the game's groups first; the offers screen is the one with no key of its own, since it opens on a crowded bump or on the interact key.
`LogPanel` and `ScrollbackPanel` have no view plugin at all: both draw `MessageLog`, one as the last few lines along the map and one as a whole scrollable screen, and neither knows the other exists.
What each collector cannot work without it declares: `GearViewPlugin`, `SheetViewPlugin` and `InspectViewPlugin` need `Registries`, `NearbyViewPlugin` and `InspectViewPlugin` need `CombatRules` for the relation a row carries, and `OffersViewPlugin` and `OffersPanel` depend on props.
What is optional is read as optional: `VitalsViewPlugin` reads `NoiseHeard` through `reads`, so a game with no noise shows no reading rather than failing to start.

## The model

`Row` is one entity as a panel reads it: the `entity` itself, the `label` from its `Name`, its own `Glyph`, a Chebyshev `distance`, an optional `relation` and `health`, an optional `alert`, and the `facets` a game pushed.
The glyph is content rather than theme, which is why a green slime stays green in every palette, and `relation` and `health` are optional because a thing on the floor has neither.
`Alert` is `Unaware`, `Searching` or `Hunting`, three readings and no more because three is what the engine can say without guessing; what each is *called* is the presenter's, since one game's monsters sleep where another's stand idle.
`Bar` is a label, a value, a maximum and a tone, and `fraction` is how full it reads.
A view holds no `Color`, no `Rect` and no string the game did not supply, so the same data serves the terminal panels, a game's own drawing and a test that never opens a window.
A `Facet` is what the engine cannot know: a key interned in `Facets`, the words, and a tone.
The engine fills the row in `ViewSet::Collect`, a game's system pushes in `ViewSet::Annotate`, and the presenter prints what it finds in the order it was pushed.
Facets are an escape hatch and their use is a signal: two games pushing the same key means the field belongs in the view.
`ToneId` is a semantic role interned in `Tones`, and `Palette` is the colour per role.
Eleven are interned before anything is authored, `text`, `muted`, `good`, `bad`, `notice`, `title`, `frame`, `surface`, `select`, `hit` and `kill`, and their ids are constants so nothing looks them up.
A tone with no colour falls back to `text` rather than panicking, and every uncoloured tone is named once at `OnEnter(Playing)`, which is visible in a playtest and cannot spam a frame.
`readable` is what keeps a dark name legible on a dark surface without losing its hue.
`Modals` is the stack of open screens, innermost last, and empty means the world has input.
`modal_is(id)` gates a screen's own systems and `no_modal` gates the engine's, which is how a game stops discovering that its player walks while the bag is open.
A stack rather than a return-to slot, because a slot can be pushed twice and lose the first target.
`Sighted` is refilled twice a frame rather than once, at the head of the input phase and again in `ViewSet::Sight`, because those are two different moments: the turns run between them, and a list collected before the player's key was resolved is not the list the panels draw.
`InSight` reads it as actors nearest first and then things nearest first, which is the order the nearby rail prints.
`Focus` is the one entity picked out of that list, held by entity rather than by cell so two things on a tile are two stops, and a focus on something that has left sight is treated as none rather than as an error.
The rail highlights it, the look cursor opens on it and an aim opens on it when the aim can take it, so the row picked out and the thing aimed at are one choice.
`VitalsView` is the player: a label, a list of `Bar`s, armor, status badges, game facets, the turn, the position, whether the player is seen and how loud it has been.
`NearbyView` is `actors` and `things` as `Row`s with the `focused` `Sighting`; `GearView` is a `GearSlot` per registered slot in declared order, filled or empty, since what is not worn reads as clearly as what is, with a worn thing's charges when it holds more than one.
`InspectView` is where the cursor is, what the ground there is called, whether it burns, what gas hangs there, the `Row` under it and a `Duel` fought at the distance the cursor stands from the player.
Its collector builds `blows` and `shots` for both sides, packs each pair with `Loadout::arms` and hands that Chebyshev gap to `Combatant::armed`, so an actor carrying only a gun reads dangerous across the room and harmless once you are beside it.
A `Prop` is named and never duelled, since a crate's health is there to be broken rather than fought, and `is_a_threat` is what a presenter of a game's own asks when it wants the forecast only against something the player is at odds with.
`AbilityView` is an `AbilityRow` per ability the turn-holder knows, in registration order so a key bound to the third row stays bound to it, each carrying its costs, requirements and effects as sentences and every reason it is refused.
`TargetView` is what is being aimed, the cursor, the footprint, the flight, what lies beyond it, whether the aim is legal, why not, and a `Row` per target.
`AimAt`, `AimThrow` and `AimFire` are how a key asks for a cursor, and the cursor writes the `Intent` itself on confirm.
`OffersView` is an `OfferRow` per verb the player is offered where it stands, with its cost in hundredths and the reason a refused one is refused.
`SheetView` is the character sheet: stats with every `Change` that made them what they are, resists, strikes, statuses and what is worn.
A presenter reads a view, reads the `Palette`, and writes cells to the `Terminal`; it owns no state and makes no decision a game might want made differently.
`panel::split_right`, `split_bottom` and `split_top` hand back both halves of a cut, which is the whole of the engine's opinion about layout: no resource holds every panel's rectangle.
`clear`, `frame`, `section` and `bar` are public so a game taking the view and drawing its own does not rewrite a box-drawing routine, and `ListMenu` is the selection a screen keeps.
A strip clips a long line and a screen wraps it, because on a strip a cut line is a cut line while on a screen the reader opened in order to read.

## Using it

Adding panels is the whole of the cheapest way in: each plugin holds its rectangle, reads a view the engine keeps current, and draws itself.

<!-- include: ../../../../examples/tutorial/src/bin/step10_panels.rs:panels -->
```rust,no_run
        // Five panels. Each holds its own rectangle, reads a view the engine
        // keeps current, and draws itself: none of them needs a system here.
        // Warren has no equipment slots, so it takes no `GearPanel`. Opt-in
        // is per panel: you add the ones you have a game for.
        .add_plugins((
            VitalsPanel::new(screen.vitals).heading("Vitals").bars(10),
            NearbyPanel::new(screen.nearby).titled("").headings("In sight", "On the floor"),
            LogPanel::new(screen.log),
            InspectPanel::new(screen.inspect),
            // A second presenter over the log the strip already draws: `p`
            // opens all of it, scrollable and filterable by tone.
            ScrollbackPanel::new(screen.scrollback),
            // Every key declared below and by the engine, on one screen, and
            // the one hint that opens it in the rail's last row.
            ControlsPanel::new(screen.controls).hint(screen.hint),
        ))
        // What the engine cannot know about a row. Named by set, never by
        // ordering after a collector function.
        .add_systems(Update, (note_bag_and_floor, note_what_a_rat_is_doing).in_set(ViewSet::Annotate))
```

What the engine cannot know arrives the other way, as a facet pushed onto a row in `ViewSet::Annotate`.

<!-- include: ../../../../examples/tutorial/src/bin/step10_panels.rs:annotate -->
```rust,no_run
/// What a rat is up to, on the row the engine built for it.
///
/// `MonsterAIMode` is not a thing the engine has; `flee_at` is this
/// game's rule. So the row gets a facet, in a tone this game declared,
/// and the rail prints it without knowing what fleeing is.
fn note_what_a_rat_is_doing(
    mut nearby: ResMut<NearbyView>,
    mut facets: ResMut<Facets>,
    tones: Res<Tones>,
    bestiary: Res<Bestiary>,
    rats: Query<(&Kind, &Health)>,
) {
    let fleeing = tones.get("fleeing").expect("declared while building");
    for row in nearby.actors.iter_mut() {
        let Ok((kind, health)) = rats.get(row.entity) else { continue };
        if health.current <= bestiary.defs.get(kind.0).flee_at {
            row.facets.push(facets.facet("mood", "fleeing").toned(fleeing));
        }
    }
}
```

## The line

The engine owns the query and the game owns the look, and every escape from the look is cheaper than the one below it: retitle and move the rectangle, swap the palette, keep the view and write the presenter, or add neither plugin.
Opt-in is per panel and not per crate, so a game with no equipment adds no gear panel and nothing in it ever runs.
A widget takes a `ToneId` and never a `Color`, because a widget that took a colour is a widget every game forks; there are no literals in a presenter and no colour in a view.
No engine type, doc or constant says weapon, spell or monster: a game's vocabulary reaches a panel as a `Name` on an entity or a `Facet` on a row, and nothing in between learns a word.
The engine will not guess a name, so a game that renames a thing and forgets the `Name` shows a stale row, which is the honest cost of a component over a trait a game would have to implement to hand back one string.
Arithmetic that is really about the rules is not a panel's: the inspect panel's forecast is `rl_rules::forecast`, resolved through the same mitigation pipeline a real blow goes through, so a duel that reads wrong is a rules bug with a test rather than a drawing bug.
Which of the two attacks that forecast counts is the resolver's own rule, held in `Arms::at`: the panel hands over both sets of rolls and the gap the two stand at and picks nothing for itself.
A game's annotate system names `ViewSet::Annotate` and never orders itself after a collector function, so the engine may split a collector in two without breaking it.
Views are rebuilt every frame rather than on a turn boundary: a frame already rewrites every cell of the map, and a panel that is one turn stale is the kind of bug that survives to a release.
The engine never opens or closes a modal; a key handler does, and the run conditions read the stack.
What the engine does not get is a widget toolkit: no text entry, no scrollbars, no drag, no focus traversal, and no main menu, settings or key-rebinding screen, because those belong to an application rather than to a roguelike.
The test for whether a panel belongs here is whether it needs engine state to build one: the nearby list needs the viewshed, and a settings screen needs nothing.
A view carries an `Entity`, which is why views are tier 2 and only their arithmetic is not.

## Where it lives

`rl-rules` is tier 1 and holds the derivations, so `expected_damage`, `blows_to_fell`, `turns_for` and `duel` are property-tested over plain numbers with no `App`, and are as usable by a balance report as by a screen.
`rl-ui` is tier 2 and holds everything that names an `Entity`: `view/` is the data and the collectors, `panel/` is one terminal presenter each, and `tone.rs`, `facet.rs`, `modal.rs` and `focus.rs` are the four small registries they all share.
Keeping the presenters in a module of their own is what lets two of them draw one view, and what lets a game delete all of them and keep the data.
`rl-render` sits below and owns the surface: a presenter writes `Cell`s into a `Terminal`, and one that paints over the map asks `MapView` where a tile sits.
`rl-bevy` gains nothing from any of this; it fixes `PresentSet`, and the components a collector reads are the ones the engine already had.
