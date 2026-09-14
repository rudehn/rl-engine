# Lighting

Status: phases A to C built 2026-09-11, with a Brogue-style renderer and Corsair's dark caves; the rest proposed.
Written 2026-09-11 against `main` at `7df090a`.
It adapts the fantasy-rogue lighting plan (v3) to the engine's shape: theme-agnostic, opt-in, split across the tiers, and integer throughout.

## 0. Summary

Light is a second field beside the field of view.
Geometry decides what an observer could see; light decides what reaches its eye.
A tile is seen when it is in the observer's shadowcast and one of these holds: it is lit at or above a threshold, it is within the observer's dark sight, or it is adjacent.

The three decisions that shape everything else:

1. **The gate lives in `update_viewsheds` and nowhere else.**
   Every consumer of `Viewshed::can_see` already means "actually seen" (the map view, the glyph filter, the minds).
   Gate at the one writer and they are all light-correct at once.
2. **Brightness and colour are separate channels.**
   Gameplay reads `intensity: u8`; the renderer reads both.
   Deriving brightness from luminance makes red light count as darkness, so hue would be setting difficulty.
3. **Lighting is opt-in, like the surface.**
   No `Lighting` resource means no gate, no cost, and no change to any game built so far.
   Corsair's daylit islands and the delve's floors both keep working until each chooses to turn the lights on.

The engine never names a torch, a sun or lava.
It knows emitters and one ambient level per map, which the game sets and may leave constant forever.
A day and night cycle is not part of the engine: a game that wants one writes the ambient from the clock in a system of its own, and a dungeon delve never sees the idea.

## 1. What lighting buys

These are the features it unlocks, roughly in the order they become cheap.

- **Darkness as the dungeon's default.**
  A floor is revealed by the light you carry, not by a sight radius.
  Exploration becomes a resource question: how far do you go on what you have.
- **Stealth, in both directions, for free.**
  An unlit monster in the dark is not in the viewshed, so it is invisible without a notice roll.
  A lit player is seen from further than an unlit one.
  Dousing a light to sneak past something becomes a real move.
  This falls out of the gate; no stealth system is needed to get it.
- **Light on items, monsters and props alike.**
  One component does all three: a brazier is a prop that never moves, a wisp is a monster that carries its own glow, and a torch lights the floor where it lies and the carrier's tile once picked up.
  A lantern-bearing sentry is a monster the player sees coming and that sees the player coming too.
- **Day and night on the open world, for the games that want it.**
  Ambient is a per-map value; a game may drive it from the turn clock so the surface darkens at night for nothing per tile.
  Ports glow from their huts; camps from their fires; the wilds go black.
  A delve sets ambient once and never thinks about it again.
- **Coloured atmosphere per biome and per map.**
  Bile pools glow green, the whale's heart pulses red, a crystal cave is cyan, a port is amber.
  Ambient intensity is also the cheapest biome differentiator in the engine.
- **Light as a tool.**
  Drop a lit thing to light a choke point and retreat; throw one once throwing lands; light a room to see what is in it at the cost of being seen.
- **Light-averse and dark-sighted creatures.**
  A mind tactic that refuses to path into intensity above a threshold turns a dropped torch into a portable wall.
  Dark sight on a hunter makes darkness its territory.
- **Authored light in prefabs.**
  A sconce is a prefab mark, and marks already come out of a chain as `Spot`s.
  A room lit by its braziers and a corridor left dark is one pass and no new machinery.
- **Later: fire and gas that glow, darkness as a spell.**
  When tile fields land, burning tiles inject into the static layer directly.
  A negative emitter subtracts intensity, which is why a shadow layer is reserved now and not retrofitted.

## 2. The model

```rust
/// One tile's light, as gameplay and the renderer read it.
pub struct Light { pub intensity: u8, pub color: Rgb }

/// A point source.
pub struct Emitter { pub origin: Point, pub intensity: u8, pub radius: i32, pub color: Rgb }
```

**Falloff** is integer and continuous: full strength within `radius / 3`, linear to exactly zero at the rim.
Reaching zero at the rim is what stops the lit edge from aliasing and stops a tile from flickering across the threshold on rounding.

**Blending** two sources on a tile is a screen blend, per colour channel and separately on intensity.
Two lanterns light a room better than one, and nothing exceeds 255.
Integer screen blending truncates on every pairwise call, so it is not associative.
Emitters are sorted by origin then intensity before they are applied, so the field is byte-identical regardless of ECS iteration order.
A property test shuffles the input and asserts an equal field.

**Reach** is the symmetric shadowcast that already exists: light reaches a tile when the emitter can see it.
Walls are lit from the side that faces the source, which is what the existing wall rule already gives.

**Ambient** is one `Light` per map, screen-blended in last.
Sunlight is not a point source; it is a floor over the whole map, so a daylit surface costs nothing per tile.
Ambient is data the game writes, not a rule the engine runs: a constant for a delve, a curve over the clock for an open world.

## 3. Where each piece lives

### rl-grid: `light.rs` (tier 1, no Bevy)

- `Light`, `Rgb`, `Emitter`, `falloff`, `screen`.
- `LightField`: a `Grid<Light>` over the window with `clear`, `at`, `cast(&impl OpacitySource, &Emitter, scratch: &mut BitGrid)`, `flood(rect, light)` for contiguous hazards that must not shadowcast one by one, and `compose(&statics, &dynamics, ambient) -> &mut self`.
- Nothing allocates per cast: the scratch `BitGrid` and the field are owned by the caller and reused.
- A criterion bench at 20 sources on the realistic maps, alongside the FOV benches.
  The plan promised this bench from day one and it is still unmeasured.

### rl-bevy: `lighting.rs` (tier 2)

- `LightSource { intensity, radius, color }`, a component on anything with a `Position`.
  A game toggles a light by inserting or removing the component.
  The same component serves every kind of thing that glows:
  - a **prop** is a fixture entity with a `Position` and no `Actor`; it is static and only rebuilt when something changes;
  - a **monster** is an `Actor` with the component; it is dynamic and lights wherever it walks;
  - an **item** on the ground is static at its tile, and once carried it sheds from the carrier's tile, followed through `Inventory` which the item layer owns.
  Whether a thing is lit is the game's call, made by inserting or removing the component; the engine reads only what is there.
- `DarkSight(i32)`: how far an actor sees with no light at all.
  Absent means adjacency only.
- `Fuel(u32)`: optional, ticked on `TurnEnd` beside statuses; at zero the engine removes the `LightSource` and writes `LightEvent::BurntOut`.
  Fuel is the one piece of per-instance state, and it is the game's to save, as `Enchant` already is.
- `Lighting` resource: three layers over the window (static, dynamic, combined), the window origin, `ambient: Light`, and `static_dirty`.
  Inserting it turns lighting on.
  `ambient` is a plain field the game writes: once at start for a delve, or from a system of its own on the open world.
  There is no ambient rules trait and no clock hook in the engine; a game with no day cycle carries none of its machinery.
- Systems in a new `EngineSet::Light`, between `Turns` and `Fov`:
  1. `rebuild_static_light` when `static_dirty`: every `LightSource` on an entity that is not an `Actor` and is not carried.
  2. `rebuild_dynamic_light` whenever a turn progressed: every `LightSource` on an `Actor`, plus every carried item with a `LightSource`, shed from the carrier's tile.
     The item layer owns `Inventory`, so the engine can follow item to carrier; the fantasy-rogue plan could not.
  3. `compose_light` with the map's ambient.
- Static dirtying is by change detection, not by hand: `Added`/`Changed<Position>` and `RemovedComponents<LightSource>` on non-actors, `ChunkLoaded`, `MapChanged`, and the opacity epoch below.

### rl-bevy: the opacity epoch

`WorldMap::set_tile` records the edit but dirties nothing.
A door opened by a future action would not refresh anyone's view until they moved.
`set_tile` compares the old and new tile's `opaque` flag and bumps `opacity_epoch` when it differs.
Viewsheds and the static layer both watch that epoch.
This closes a gap that exists today, lighting or not, and lands in phase B regardless.

### rl-bevy: the gate

`Viewshed` gains a second bitset: `line` is the geometric shadowcast, `visible` is what is actually seen.
With no `Lighting` resource the two are the same buffer and nothing changes.
With one, `visible = line ∩ (lit ≥ threshold ∪ within DarkSight ∪ adjacent)`.
`Knowledge::mark` runs over `visible`, so a dark corridor is not remembered until it has been lit.

Minds today use the player's viewshed as a symmetric oracle.
Symmetry holds for geometry, not for light, so `decide_minds` reads `line` for "is there a sightline" and then asks the light: I see the player if the player's tile is lit, or is within my dark sight, or is adjacent.
That one change is what makes a dark-sighted hunter dangerous and a doused player hidden.

### rl-render

Built in the manner of Brogue, after Nate pointed at a Brogue screenshot on 2026-09-11.
A tile is authored with a glyph, both colours and a `Vary`: each cell is jittered by a hash of its position, and restless tiles shimmer over time.
`draw_map` multiplies both colours of a visible tile channel by channel by the light that lands, from a dark floor up to a gain cap, so light paints the background as well as the glyph.
The wavering part of the light dips on a smooth noise that neighbouring cells share, so a flame ripples instead of strobing.
Remembered tiles fade to a darker, greyer, cooler colour, so memory and sight never look alike.
Glyphs are lit the same way.
A heat-map toggle draws the intensity as digits, which is how phase A is inspected before anything else changes.

## 4. The example: lamplight

Folded into the delve on 2026-09-13, when the examples became one open world and one dungeon: the brand burns `Fuel` and can be smothered, a torch lies on the first floor, whalers' lamps stand in for the brazier, and `v` shows the light as digits.
What follows is the plan as written.

A third example, `examples/lamplight`, small enough to read in one sitting, is the first thing lighting is built against.
The delve and Corsair adopt lighting only after it works there.

- One map, one cellular cave, ambient zero, no surface, no stairs.
- The player carries a lantern: `l` inserts or removes its `LightSource`, and it burns `Fuel`.
- A brazier prop in the largest chamber: static, amber, never moves.
- A wisp: a harmless monster with a faint blue glow that wanders, so the player watches light move through the dark.
- A lurker: a dark-sighted hunter with no glow, visible only when the player's light reaches it, which is the whole lesson of the gate.
- A torch on the floor: lit where it lies, shed from the player once picked up, and lit on the floor again when dropped.
- A heat-map toggle on `v` that draws intensity as digits.
- Headless tests that walk the player into the dark and assert what is and is not seen.

It exists to show the system, so it does nothing else: no combat rules beyond what spawning a monster needs, no items beyond the two lights, no save.

## 5. Save and determinism

`Lighting` is derived and never persisted; a load marks the static layer dirty and rebuilds.
`Fuel` and the presence of a `LightSource` on an item are the game's to save with its item state.
Nothing in lighting draws from any RNG.
Flicker is cosmetic.
A source's `flicker` becomes a `waver` channel in the field, the part of a tile's intensity that may dip, and only the renderer reads it, dipping it on a noise over the frame clock.
Gameplay reads the steady intensity, so a guttering torch never changes what is seen and a replay is unaffected.

## 6. Cost

Per turn with lighting on: one shadowcast per dynamic source at its radius, a compose pass over the window, and the viewsheds the game already pays for.
The static layer is untouched on a turn where nothing opened, burned out or moved among the fixtures.
The surface window is up to nine regions, so the compose pass is the number to measure; the bench in phase A reports it.

## 7. Phases

- **A. The field, invisible.**
  `rl-grid::light`, its tests and bench.
  `Lighting`, `LightSource`, the three layers and the opacity epoch in rl-bevy.
  The heat-map overlay in rl-render.
  Nothing on screen changes for either existing example.
- **B. The gate and the lamplight example.**
  `Viewshed::line`, the threshold, `DarkSight`, the adjacency floor, the mind rule, the render tint.
  Carried sources shed from the carrier, `Fuel`, `LightEvent`.
  `examples/lamplight` lands in the same slice with its prop, monster and item lights and its tests.
  Built: the static and dynamic layers are recast when their sorted emitter lists differ from the last cast, which needs no change detection and catches every add, move, pickup and removal; facts for the ledger were left to the game, which maps `LightEvent` as it maps any other event.
- **C. The delve goes dark.** Built, with Corsair's caves in the same slice.
  Ambient zero, bile pools as static green sources, the heart as a red one, the player starting with a burning brand, salt ghosts with dark sight.
  Play it before going on; this is the phase that changes the feel of an existing game.
- **D. Corsair, at its own pace.**
  A lantern and oil in the armory, huts as static amber, caves dark.
  A Corsair-side system writes ambient from the turn clock so the open water has nights; this is content in the example, not a feature of the engine.
- **E. Mechanics on top.**
  Lit detection range for minds, a light-averse tactic, sconces as prefab marks in the dungeon passes, a ranged penalty in the dark once accuracy exists in the combat rules.
- **F. With tile fields.**
  Burning tiles inject into the static layer by flood; a shadow layer for negative emitters.

## 8. Tests

Headless, in the engine, on ASCII fixtures:

- `falloff` is `intensity` at `d <= radius / 3` and exactly 0 at `d == radius`.
- `screen` is commutative; a shuffled emitter list gives an identical field.
- A tile beyond every emitter is not visible even when in line.
- A tile behind a wall is unlit with the emitter two tiles away.
- Setting a door tile from opaque to clear bumps the epoch, dirties the static layer and the viewsheds; an actor walking past a fixture does not.
- A lightless observer in the dark sees its own tile and its eight neighbours.
- `DarkSight(4)` sees an unlit actor at 3 and not at 6.
- A mind sees a lit player across a dark room and does not see a doused one at the same distance.
- A dropped lit item keeps lighting its tile; picking it up sheds from the carrier.
- `Fuel` reaches zero, the source is removed, and the event is written once.
- Without a `Lighting` resource every existing test passes unchanged.

## 9. Risks

- **Both examples were authored assuming you can see.**
  Opt-in per game and ambient per map are the mitigation, and ambient cannot be left until the end.
- **Flicker at the rim.**
  A monster stepping in and out of a moving light's edge is the likeliest "this feels broken" report.
  The continuous falloff reaching zero removes the rounding case; a one-turn grace on anything already seen is the fallback if it still bites.
- **A game forgetting to set ambient.**
  The field defaults to an ambient of zero, so a game that inserts `Lighting` and nothing else is pitch black, which is the loudest possible reminder.
- **Dark sight defaults.**
  Absent means adjacency only, so an unauthored bestiary goes blind the moment a game turns lighting on.
  The examples author it deliberately; a game that forgets will notice in its first playtest, which is acceptable for an opt-in feature.
