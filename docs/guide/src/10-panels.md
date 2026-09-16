# Panels

> Run it: `cargo run -p tutorial --bin step10_panels`
>
> Source: [`step10_panels.rs`](https://github.com/rudehn/rl-engine/blob/main/examples/tutorial/src/bin/step10_panels.rs)

![Warren with a rail down the right: a Vitals section with a green health bar, crusts and the floor, then On the floor listing a crust of bread by its own % glyph](images/10-panels.png)

Warren has had a status line and a log since [chapter 3](03-what-the-player-knows.md).
This chapter gives it the rest: what is in sight, what is worn, and a cursor you can point at a rat to ask how the fight would go.

None of it is a widget you fill in.
Every panel reads a resource the engine rebuilds each frame, and the interesting question is which half of that you keep.

## The three layers

A panel is split in three, and the split is why any of this belongs in an engine:

| Layer | What it is | Where |
|---|---|---|
| **View** | a resource of plain data: rows, bars, numbers | `NearbyView`, `VitalsView`, `GearView`, `InspectView` |
| **Collector** | the system that rebuilds the view from the world | `ViewSet::Collect` |
| **Presenter** | one way of drawing it | `NearbyPanel`, `VitalsPanel`, … |

"Every actor in the viewshed, nearest first, with a health fraction and a relation" is the same sentence in every roguelike.
A gold-ruled rail with small-caps headings is one game's taste.
So the engine owns the query and offers the drawing.

You can stop at any of five places:

1. Add the panel and be done.
2. Change the `Palette`, and every panel restyles at once.
3. Push a `Facet` for things the engine cannot know.
4. Keep the view, drop the panel, draw it yourself from the same data.
5. Add neither. Nothing here runs unless you ask for it.

## Cutting up the screen

<!-- include: ../../../examples/tutorial/src/bin/step10_panels.rs:layout -->
```rust,no_run
/// The screen, cut up once, so the map and every panel agree on it.
///
/// `panel::split_*` take a rectangle and a size and hand back both
/// halves, which is the whole of the engine's opinion about layout.
struct Screen {
    map: Rect,
    log: Rect,
    vitals: Rect,
    nearby: Rect,
    inspect: Rect,
    scrollback: Rect,
    controls: Rect,
    hint: Rect,
}

impl Screen {
    fn new() -> Self {
        let (left, rail) = panel::split_right(Rect::new(0, 0, COLS, ROWS), RAIL);
        let (map, log) = panel::split_bottom(left, LOG_ROWS);
        let (vitals, nearby) = panel::split_top(rail, 9);
        // The last row of the rail says how to see the controls.
        let (nearby, hint) = panel::split_bottom(nearby, 1);
        // Over the map, because a modal covers what it is about.
        let inspect = Rect::new(map.x + 2, map.bottom() - 10, map.width.min(46), 9);
        Self { map, log, vitals, nearby, inspect, scrollback: map.inflate(-2), controls: map.inflate(-2), hint }
    }
}
```

`panel::split_right`, `split_bottom` and `split_top` take a rectangle and a size and hand back both halves.
The engine has no other opinion about layout.
There is no layout resource to fill in and no panel that decides where it goes: each one is told.

## Adding them

<!-- include: ../../../examples/tutorial/src/bin/step10_panels.rs:panels -->
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

Each panel is a plugin holding its own rectangle and its own headings, and each pulls its view plugin in behind it.
Warren adds no `GearPanel`, because Warren has no equipment slots: opt-in is per panel, not per crate.

Drawing is layered by `PresentSet`, so nothing orders itself after another crate's draw function.
The panels land in `Chrome`; the look cursor lands in `Overlay`, over the map it is about.

## What the engine cannot know

The engine has no bestiary, no item table and no idea what a crust is.
Two things bridge that, and Warren uses both.

**A `Name` on the entity**, and that is all it takes:

```rust,no_run
            commands.spawn((Item, Crust(8), Name::new("a crust of bread"), Position(p), Glyph::new('%', ...)));
```

Bevy's own `Name`, not a component of the engine's.
Leave it off and the collector skips the entity instead of drawing a blank row, because a nameless row is a spawn that forgot and a blank line is the hardest kind of that to notice.

**A `Facet` on the row.** A key, some words, and a tone, pushed in `ViewSet::Annotate`:

<!-- include: ../../../examples/tutorial/src/bin/step10_panels.rs:annotate -->
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

`flee_at` is Warren's rule and the engine has never heard of it.
The rail prints the facet without knowing what fleeing is.

Name the set, never the collector function: the engine is free to split a collector in two, and your system keeps working.
The vitals strip's `crusts 0` and `floor 1/4: the Burrow` arrive the same way:

<!-- include: ../../../examples/tutorial/src/bin/step10_panels.rs:status -->
```rust,no_run
/// What the engine cannot know: the bag, and which floor this is.
///
/// A facet is a note on a view: a key, some words, and a tone. The engine
/// has no idea what a crust is, and this is how it never needs one.
fn note_bag_and_floor(mut vitals: ResMut<VitalsView>, mut facets: ResMut<Facets>, map: Res<WorldMap>, player: Query<&Inventory, With<Player>>) {
    let Ok(bag) = player.single() else { return };
    let depth = floor_of(map.current());
    vitals.facets.push(facets.facet("crusts", format!("crusts {}", bag.items.len())));
    vitals.facets.push(facets.facet("floor", format!("floor {depth}/{FLOORS}: {}", name_of(depth))));
}
```

## Tones, not colours

No widget in `rl-ui` takes a `Color`.
They take a `ToneId`: a semantic role, interned, that the `Palette` turns into a colour.
The engine ships nine roles (`text`, `muted`, `good`, `bad`, `notice`, `title`, `frame`, `surface`, `select`) and a game adds its own:

<!-- include: ../../../examples/tutorial/src/bin/step10_panels.rs:tone -->
```rust,no_run
    // A role the engine never heard of, and the colour for it. Every
    // widget that takes a tone honours it from here on.
    app.add_tone("fleeing", Color::srgb(0.6, 0.8, 1.0));
```

Warren's fleeing rats read in a pale blue nothing in the engine has an opinion about.
`add_tone` declares the role and colours it in one call, and a tone declared any other way and never coloured is named by a warning at startup, instead of the rows coming out in the text colour with nothing said.

## One screen at a time

The look cursor is a modal, and so is anything you add.

<!-- include: ../../../examples/tutorial/src/bin/step10_panels.rs:gate -->
```rust,no_run
        // One gate for every screen there is and every screen added later:
        // the stack is empty, or the world does not have the keys.
        .add_systems(Update, player_input.in_set(EngineSet::Input).run_if(no_modal))
```

`Modals` is a stack of interned ids. `no_modal` is true when it is empty, `modal_is(id)` when that one is on top.
One gate on `player_input` covers the look cursor and every screen Warren might grow later, and the player cannot walk with a screen up.

An action that spends a turn calls `close_all`, because the turn loop assumes nothing is open.

With no screen up, `tab` steps down the rail and lights the row and its tile on the map, and `shift` with it steps back.
`Focus` is the engine's own, the one thing picked out of what is in sight, and the look cursor opens on it and moves it as it goes.
Warren wrote none of it: `NearbyPanel` and `InspectPanel` share it through the same list.

## Every key, once

Warren declares its keys in one place and reads them by name.

<!-- include: ../../../examples/tutorial/src/bin/step10_panels.rs:keys -->
```rust,no_run
/// Every key Warren answers to, by name.
///
/// The names are what `player_input` checks; the keys behind them are
/// declared once in `declare_controls`, and the `?` screen lists that
/// same declaration. A key the game stops reading leaves the screen with
/// its declaration.
#[derive(Resource, Clone, Copy)]
struct Binds {
    walk: ControlId,
    shove: ControlId,
    stairs: ControlId,
    pick_up: ControlId,
    eat: ControlId,
    wait: ControlId,
    quit: ControlId,
}

/// Declares the keys, under the headings the `?` screen groups them by.
///
/// The walk is every direction key the engine binds, arrows, `hjklyubn`
/// and the numpad; the shove is the same keys with Shift held. A chord is
/// matched exactly, so `L` never reads as a step east.
fn declare_controls(app: &mut App) {
    let binds = Binds {
        walk: app.add_control("Move", "walk, or strike whoever is there", Keys::Directions { shift: false }),
        shove: app.add_control("Move", "shove whoever is there", Keys::Directions { shift: true }),
        stairs: app.add_control("Move", "take the stairs", [Chord::key(KeyCode::Enter), Chord::shift(KeyCode::Period), Chord::shift(KeyCode::Comma)]),
        pick_up: app.add_control("Act", "pick up what is here", KeyCode::KeyG),
        eat: app.add_control("Act", "eat a crust", KeyCode::KeyE),
        wait: app.add_control("Act", "wait a turn", [KeyCode::Period, KeyCode::Numpad5]),
        quit: app.add_control("Game", "quit", KeyCode::KeyQ),
    };
    app.insert_resource(binds);
}
```

`player_input` asks `keys.direction(binds.walk)` and `keys.just_pressed(binds.eat)`, never `KeyCode::KeyE`.
`ControlsPanel` lists the same registry, so `?` shows exactly the keys Warren reads, and a key Warren stops reading leaves the screen with its declaration.
The engine's own screens declare theirs the same way, the look cursor's, the log's and `?` itself, and list them from the resources that bind them, so a game that moves "look" off `x` sees the new key on the screen without telling anyone.
A `Chord` is a key with Shift held or not, matched exactly: the shove is Shift with a direction, and `L` is never read as a step east.

Steps 2 to 9 read `KeyCode`s straight off `ButtonInput` in `player_input`, and that is the right size for a game with four keys and no screen to list them on.
The registry pays for itself here, where the `?` screen arrives, and everything Warren grows from now on is declared into it.

## Two presenters, one view

Press `p` and the whole log opens, scrollable, ruled off by turn, and filterable by tone with `tab`.

```rust,no_run
        ScrollbackPanel::new(screen.scrollback),
```

Nothing about the log changed to make that work.
`LogPanel` draws the last few lines along the bottom and `ScrollbackPanel` draws all of them on a screen, over the same `MessageLog`, and neither knows the other exists.
The scrollback keeps its own cursor and filter in a `Scrollback` resource, because where you have scrolled to is not something the log should know.

It wraps its lines with `panel::wrap` instead of clipping them.
Your own presenter would want the same function.

## What the forecast is made of

Point the cursor at a rat and the panel says how the fight goes: how many turns to fell it, how many for it to fell you, and a word for the two together.

Those numbers are not the panel's arithmetic.
`rl_rules::forecast` puts the average roll through the same mitigation pipeline a real blow goes through, your `DamageStages` included, so the forecast cannot drift from the fight.

## Try it

- Change one entry in the `Palette` and watch every panel follow.
- Drop `NearbyPanel` and keep `NearbyViewPlugin`, then draw the rows yourself with `panel::bar` and `panel::section`.
- Give a rat no `Name` and watch it vanish from the rail while staying on the map.
- Push a facet keyed `mood` from two different systems and see both print, in order.
- Log fifty lines, open `p`, and hold `tab`: the filter offers only the tones Warren actually logs in.
- Press `tab` twice with nothing open, then `x`: the look cursor opens on the second row, not the nearest rat.
- Press `?`. Then move `CursorKeys::look` to another key in `main` and press `?` again: the screen followed, and nothing in Warren mentioned it.

Next: [testing](11-testing.md).
