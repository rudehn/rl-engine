# Landmarks: prefab facings and seeded first pieces

Status: written 2026-10-13, from a design interview with the user; not yet built.
It is the engine half of the work `2026-10-13-undermere-strata-design.md` sets out, and Undermere is its first and, for now, only user.
The layout it extends is `docs/design/layout.md`; the idea is Cogmind's "seeding" (`docs/research/gridsage-mapgen.md` section 5.2), joined to the floor the way Brogue and the engine's accretion already join every piece.
Foundry is out of scope: nothing here is wired into it.

## Goal

A floor should be remembered by something on it.
Today the accretion grower lays its first piece at a random point and attaches every later piece to a wall of what already stands, and the pieces a floor must hold are laid late, into space already grown.
Nothing large is put down first, at a chosen place, for the floor to grow around, so no floor has a heart and the stairs stand wherever the survey finds them.

This work adds two things, each generic:

1. **Facings a prefab declares**, so every prefab, not only a landmark, says how far it may be turned and mirrored.
2. **A seeded first piece**, a landmark, laid by a placement rule before growth, with the floor grown outward from the openings drawn on it and the stairs set by a rule relative to it.

## 1. Facings a prefab declares

### Today

`rl_mapgen::prefab::Orient` is `Fixed`, `Turned` (four quarter-turns) or turned and mirrored (eight facings), and it is chosen per stamp, not per prefab.
Its doc gives the reason: the same vault may be free to turn in a cave and fixed against a corridor that has to meet its door.
`Prefab::rotated` and `Prefab::flipped` (left to right) exist, and marks and entries turn with the tiles.
There is no up-down mirror; with quarter-turns allowed it is redundant, since a half-turn and a left-right mirror make one, but a piece that may not turn at all cannot ask for it.

### The change

A prefab's definition declares what its drawing tolerates:

- `turn: None | Half | Quarter`, none, the half-turn, or all four quarter-turns.
- `mirror: None | LeftRight | UpDown | Both`.

The facings a prefab may land in are every combination of what it allows, deduplicated as `Orient::facings` already does for a symmetric piece.

The stamp keeps its say, and may only narrow: a place that needs the piece fixed against a corridor narrows it to `Fixed`, and a stamp can never widen a piece past what its drawing declares.
Both reasons are real and only one of each side knows its own: the drawing knows an inscription reads backwards mirrored and a ship should not stand on its mast, the place knows where its corridor meets the door.
So `Orient` on a stamp becomes a ceiling, and the landed facings are the intersection of the stamp's ceiling and the prefab's declaration.

Every existing prefab gets a declaration in the same change.
A prefab with none declared defaults to what its call sites stamp it with today, so that no layout changes until a drawing says otherwise; the fingerprint tripwires should not move in this step.

The RON schema comment at the top of each prefab file lists the full option space, as every schema does.

## 2. A seeded first piece: the landmark

### What it is

A landmark is not a new kind of piece.
`rl_mapgen::prefab::PrefabShape` already makes a prefab an accretion `Shape`: an area of its own, entered only at the cells its legend draws as `Cell::Entry`, never taken back for having one way in, and published as a `Stamped` so its marks are filled as any stamp's are.
`Accrete::pieces` already grows such pieces, but late, once the open share passes `late_pct`, attached to whatever stands then.

What is new is only where it goes: `Accrete` gains an optional first piece, a shape and a placement.
When it is set, the grower's first-piece step (`accrete.rs`, "The first piece, anywhere it fits") lays that shape at the placement instead of a district's shape at a random point, and growth then proceeds exactly as today, every later piece attached to a wall of what already stands.
The landmark's `Cell::Entry` cells are where the floor reaches it, so the floor is connected by construction and the side a landmark is reached from is the side its drawing opens on; no new mark is needed for its openings.

Cogmind's alternative, walling the landmark off, growing the floor separately and then tunnelling from marked edges until they meet, was considered and refused: it needs a tunnelling pass and a connectivity repair the accretion grower never needs.

### Placement

`rl_mapgen::prefab::Placement` already says where a stamped piece goes: `Center`, `At(point)`, `InRoom(index)` and `AnyRoom`.
It gains three variants, and the first piece takes a `Placement` rather than a vocabulary of its own:

- `Center`, as today: centred on the map.
- `Edge`: against one side of the map, which side drawn from those the piece's facings allow.
- `Anywhere`: at a random point, kept a margin from every edge.
- `Span`: a long piece laid across the middle of the map along one axis.

The room variants mean nothing before growth, since no room exists yet, so the first piece refuses them with an error naming the variant; a stamp after growth may use the new three as well as the old four, which is the point of one enum.

`Edge` is a convention, not a mechanism: a piece meant for an edge is drawn with its back to the south, and laying it on the north edge turns it a half-turn, on the east or west a quarter-turn.
An edge whose turn the drawing's facings do not allow is never drawn for it, and loading refuses an `Edge` piece whose facings fit no edge at all.

`Span` is a fixed-size drawing; it does not stretch to the map.
The floor is carved round it and most routes will pass through it, but a floor that grows a way round its ends is acceptable.
Stretchable drawings, a marked band repeated to fit, were considered and set aside as more prefab design than the case needs.

### Stairs

The way in and the way down are already chosen by passes in the `Exits` phase: `rl_mapgen::dungeon::RandomStart` and `FarthestExit`, each with modifiers such as `clear_of_stamps(cells)` and `among_farthest(pct)`, emitting `StartPoint` and `ExitPoint`, which `PlaceBuild` returns as its entry and exit for the ladder to lay its stairs on.
The three stairs rules are those passes configured, not a system beside them:

- `Free`: the chain as today.
- `Beyond`: a new modifier on both passes, keeping each to one side of a `Stamped`, the way in to one and the way down to the other, so the route between them passes it.
  For a `Span` the sides are the two halves it divides; otherwise they are opposite half-planes through its centre.
- `Within`: the way down is a cell the landmark's drawing marks.
  Undermere already has this pass for the way in, `MarkedStart` in `examples/undermere/src/dungeon.rs`, which emits the cell a stamp marked with a given character as the town's `@`.
  It moves into `rl_mapgen::dungeon` beside its pair, `MarkedExit`, and Undermere's copy goes.

A recipe's existing `require.route` bound still applies to every floor, landmark or not.

### How it is chosen

A landmark is a pool entry of kind `"landmark"`.
Kinds are already plain strings in `rl_rules::pool`, so the new thing is a way of being placed: alongside `Grown`, `Inside` and `Dressed`, an entry may be `Seeded(placements, stairs)`.
A run draws its landmarks with every other pick, once, at its start, so a landmark's `per_run` limit, one by default, keeps any run from seeing the same one twice, and a floor grown again after a failed attempt is a different layout round the same landmark.
A budget row of kind `"landmark"` gives a floor exactly one when its place's tags fit some entry, and none when no entry fits; a theme with no landmarks drawn yet simply has none.

A seeded piece occupies the grower's first-piece step rather than adding a step, and a `Seeded` pick is that step's shape, where `Grown` picks go to `Accrete::pieces` as they do today.

### What the survey must still guarantee

A landmark is a piece like any other once laid: the survey counts it among the areas, the loops pass may cut doors into it where its drawing allows openings, and the hidden-things pass may hide what it hides.
A landmark that leaves a recipe unable to meet its `require` bounds fails the attempt, and the floor is grown again from the next attempt with the same landmark at a fresh placement.
A landmark that fails every attempt is a content fault, and the load or a sweep test says which.

## Testing

- Facings: each declaration yields exactly the facings it names, a stamp ceiling never widens them, and a prefab with no declaration lands in the facings its stamps gave it before.
- Placement: over a seed range, a `Center` landmark's centre lies within its margin of the map's, an `Edge` landmark touches its edge with its back, a `Span` crosses the map's middle on its axis.
- Stairs: over a seed range, the `Beyond` modifier puts the way in and the way down on opposite sides, `MarkedExit` puts the way down on the drawing's mark, and Undermere's town still starts on its `@` through the moved `MarkedStart`.
- Growth: every floor grown round a landmark is connected and meets its recipe's bounds, swept over seeds for every landmark in a game's pool, as part 4's sweep does for every piece.
- Pool: no run holds one landmark twice, and a floor with no fitting entry has no landmark and grows as it does today.

## Not in this work

- Stretchable prefabs.
- A procedural landmark, grown by code rather than drawn; it is the next step once a drawn one is seen too often.
- Route decisions as a floor rule (two ways to the stairs of different kinds), and a forked or branching ladder.
- Foundry.

## As built

Built 2026-10-13 on the `landmarks` branch, in two commits, facings first.
Where the build differs from the text above, or settles what it left open:

- **Facings are closed under composition.** A declaration's facings are every facing its allowed turns and mirrors reach applied one after another, so `turn: None, mirror: Both` is four facings, the half-turn among them, since both mirrors together are one.
  A prefab that declares nothing allows all eight, so the stamp's ceiling alone decides, and every prefab file of Undermere and Foundry also declares what its chains stamped it with: `Quarter`/`Both` for what is stamped turned or grown, `None`/`None` for what is stamped fixed.
- **A narrowed ceiling draws once.** `Orient::apply` keeps its old draws whenever the drawing allows everything the stamp does, which is what keeps every map unmoved; when the drawing narrows the stamp, it draws one facing evenly among what is left, and none when one is left.
- **A grown piece has no stamp ceiling.** `PrefabShape` lands in whatever its drawing declares, as it landed in all eight before; nothing stands over growth to narrow it.
- **The first piece is a drawing, not any `Shape`.** `FirstPiece::new(name, &prefab, placements, orient)` takes a `Prefab`, since an `Edge` or a `Span` places by which way the piece faces, which a `Shape`'s random draw cannot say.
  It precomputes its facings as `PrefabShape` does and refuses a room placement, an empty list and a drawing with no entry by name.
- **A landmark has no join of its own.** The pool's `Seeded(placements, stairs)` names none, so each way into a landmark is opened by the join of whatever grows against it, through its entries only; a landmark meets the floor in the floor's own grammar.
- **Placement details.** `Edge` lays the drawing's back on the first row or column inside the border, at a random point along the edge kept 15% of the side from each corner; `Anywhere` keeps 15% of each side from every edge; `Span` is centred, its axis the facing's longer side.
  `StampPrefab` takes all three too, drawing the facing with the place.
- **Loading cannot refuse an `Edge` piece for fitting no edge.** Every declaration allows the drawing as drawn, back to the south, so every piece has the south edge; the check the text asks for has nothing to catch.
- **Sides are beyond the piece's band, not half-planes through its centre.** `Landmark::side` puts a cell on a side only when it lies wholly past the piece's bounds across the divide, so the route between a way in and a way down on opposite sides crosses the whole piece.
  The divide is across a `Span`'s length, along the edge for an `Edge` piece (a piece on the south edge divides west from east), and across the map's longer side for `Center`, `Anywhere` and `At`.
  `beyond` with no landmark laid is the plain pass from the same stream, so a chain may ask for it on every floor.
- **`Stairs` lives in `rl_mapgen::dungeon`.** Its `start(ClearStart)` and `exit(FarExit)` wrap a game's own passes, the exit as a `WayDown` pass, so a game keeps its own clearance and share; `Within` is for the way down only, and the load checks its glyph is a slot the prefab draws.
- **Seeded kinds.** The engine does not know the kind `"landmark"`: the load requires every seeded entry to be of one kind, required on every budget covering its floors, with no budget giving a floor more than one.
  `rl_bevy::layout::picked` returns the landmark as `Picked::first` and its stairs as `Picked::stairs`.
- **Undermere is wired and holds none.** It takes the engine's `MarkedStart`, puts `picked.first` into its accretion and passes its start and exit through `picked.stairs` on every floor but the throne's, which with no seeded entry in its pool is the chain it had, stream for stream.
  The sweep over every landmark in a game's pool belongs with the slice that gives it landmark content; the engine's own sweeps grow round test landmarks at every placement, connected, in `rl-mapgen` and through a pool in `rl-bevy`.
