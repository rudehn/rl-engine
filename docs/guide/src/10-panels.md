# Panels

> Run it: `cargo run -p tutorial --bin step10_panels`
> Source: [`step10_panels.rs`](https://github.com/rudehn/rl-engine/blob/main/examples/tutorial/src/bin/step10_panels.rs)

![Warren with a rail down the right: a Vitals section with a green health bar, crusts and the floor, then On the floor listing a crust of bread by its own % glyph](images/10-panels.png)

Warren has had a status line and a log since chapter 3.
This chapter gives it the rest: what is in sight, what is worn, and a cursor you can point at a rat to ask how the fight would go.

None of it is a widget you fill in.
Every panel reads a resource the engine rebuilds each frame, and the interesting question is which half of that you keep.

## The three layers

A panel is split in three, and the split is the whole reason any of this is in an engine:

| Layer | What it is | Where |
|---|---|---|
| **View** | a resource of plain data: rows, bars, numbers | `NearbyView`, `VitalsView`, `GearView`, `InspectView` |
| **Collector** | the system that rebuilds the view from the world | `ViewSet::Collect` |
| **Presenter** | one way of drawing it | `NearbyPanel`, `VitalsPanel`, … |

"Every actor in the viewshed, nearest first, with a health fraction and a relation" is the same sentence in every roguelike.
A gold-ruled rail with small-caps headings is one game's taste.
So the engine owns the query and offers the drawing.

That gives you five places to stop, and you can stop at any of them:

1. Add the panel and be done.
2. Change the `Palette`, and every panel restyles at once.
3. Push a `Facet` for things the engine cannot know.
4. Keep the view, drop the panel, draw it yourself from the same data.
5. Add neither. Nothing here runs unless you ask for it.

## Cutting up the screen

```rust,no_run
{{#include ../../../examples/tutorial/src/bin/step10_panels.rs:layout}}
```

`panel::split_right`, `split_bottom` and `split_top` take a rectangle and a size and hand back both halves.
That is the entire extent of the engine's opinion about layout.
There is no layout resource to fill in and no panel that decides where it goes: each one is told.

## Adding them

```rust,no_run
{{#include ../../../examples/tutorial/src/bin/step10_panels.rs:panels}}
```

Each panel is a plugin holding its own rectangle and its own headings, and each pulls its view plugin in behind it.
Warren adds no `GearPanel`, because Warren has no equipment slots: opt-in is per panel, not per crate.

Drawing is layered by `PresentSet`, so nothing orders itself after another crate's draw function.
The panels land in `Chrome`; the look cursor lands in `Overlay`, over the map it is about.

## What the engine cannot know

The engine has no bestiary, no item table and no idea what a crust is.
Two things bridge that, and Warren uses both.

**A `Name` on the entity.** That is the whole of it:

```rust,no_run
            commands.spawn((Item, Crust(8), Name::new("a crust of bread"), Position(p), Glyph::new('%', ...)));
```

Bevy's own `Name`, not a component of the engine's.
Leave it off and the collector skips the entity rather than drawing a blank row, because a nameless row is a spawn that forgot and a blank line is the hardest kind of that to notice.

**A `Facet` on the row.** A key, some words, and a tone, pushed in `ViewSet::Annotate`:

```rust,no_run
{{#include ../../../examples/tutorial/src/bin/step10_panels.rs:annotate}}
```

`flee_at` is Warren's rule and the engine has never heard of it.
The rail prints the facet without knowing what fleeing is.

Name the set, never the collector function: the engine is free to split a collector in two, and your system keeps working.
The vitals strip's `crusts 0` and `floor 1/4: the Burrow` arrive the same way:

```rust,no_run
{{#include ../../../examples/tutorial/src/bin/step10_panels.rs:status}}
```

## Tones, not colours

No widget in `rl-ui` takes a `Color`.
They take a `ToneId`: a semantic role, interned, that the `Palette` turns into a colour.
The engine ships nine roles (`text`, `muted`, `good`, `bad`, `notice`, `title`, `frame`, `surface`, `select`) and a game adds its own:

```rust,no_run
{{#include ../../../examples/tutorial/src/bin/step10_panels.rs:tone}}
```

Warren's fleeing rats read in a pale blue nothing in the engine has an opinion about.
Declare a tone and forget the colour and a warning at startup names it, rather than the rows quietly coming out in the text colour.

A widget that took a `Color` would be a widget every game forked. That is the whole argument.

## One screen at a time

The look cursor is a modal, and so is anything you add.

```rust,no_run
{{#include ../../../examples/tutorial/src/bin/step10_panels.rs:gate}}
```

`Modals` is a stack of interned ids. `no_modal` is true when it is empty, `modal_is(id)` when that one is on top.
One gate on `player_input` covers the look cursor and every screen Warren might grow later, and the player cannot walk with a screen up.

A stack rather than a "return to" slot, because a slot can be pushed twice and lose the first target.
An action that spends a turn calls `close_all`: the turn loop assumes nothing is open.

## What the forecast is made of

Point the cursor at a rat and the panel says how the fight goes: how many turns to fell it, how many for it to fell you, and a word for the two together.

Those numbers are not the panel's arithmetic.
`rl_rules::forecast` puts the average roll through the same mitigation pipeline a real blow goes through, your `DamageStages` included, so the forecast cannot drift from the fight.
It is pure and lives in tier 1, which means it is tested without an `App` and a balance tool can call it too.

## Try it

- Change one entry in the `Palette` and watch every panel follow.
- Drop `NearbyPanel` and keep `NearbyViewPlugin`, then draw the rows yourself with `panel::bar` and `panel::section`.
- Give a rat no `Name` and watch it vanish from the rail while staying on the map.
- Push a facet keyed `mood` from two different systems and see both print, in order.
