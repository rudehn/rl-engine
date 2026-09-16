# Where to go next

Warren is a complete roguelike in about four hundred lines and uses maybe a third of the engine.
Here is the rest.

## Lighting

Opt in by inserting one resource:

```rust
    commands.insert_resource(Lighting::dark());
```

`LightSource` goes on a prop, an actor or an item, and is shed by whoever carries it.
`Fuel` burns down by the turn and reports `LightEvent::BurntOut`.
`DarkSight` is how a monster sees without one.

This is where the `Viewshed` split from [chapter 3](03-what-the-player-knows.md) starts to matter: `line` stays geometric, `visible` shrinks to what is lit, and a monster that sheds nothing is found only when a light reaches it.

`delve` turns it on below the Maw: a brand that burns down and can be smothered, a torch to carry and set down, lamps that never move, and `v` to see the light as digits.

## Stealth

`StealthPlugin` puts a roll between being seen and being noticed.
`Notice` on an observer is a certain radius, a chance beyond it, a bonus while the subject stands in light, and a memory; `Stealth` on a subject narrows both.
A monster that has not noticed you does not act on you, one that loses you searches where it last saw you, and `Watchers` answers who is watching whom for the panels.

`delve` is built around it, and Corsair's caves use it.

## Abilities

`AbilitiesPlugin` resolves abilities written as data: an aim, a shape from the targeting footprints, costs, requirements, a cooldown and a list of named effects.
The engine ships seven effects and a game registers its own with `add_effect`.
A key writes `AimAt`; the engine opens the cursor, previews what it would cover, and spends the turn.

`delve` has five, `corsair` four, and `crates/rl-bevy/tests/genres.rs` loads five genres of them into one registry.

## Statuses, stats and gear

`StatusPlugin` ticks afflictions by the turn through the damage pipeline, with stacking rules and cures from a registry.
`rl-rules` also has a stat block with a modifier accumulator, an equipment slot graph with displacement, and an affix model: prefixes and suffixes with level-scaled grants, weighted rolling and per-instance state.
What wearing an item does goes on the item: a worn blade carries the `MeleeAttack` it is swung with, a coat its `Armor`, and an affix what it `Bestows` on a stat; `Loadout` sums them at the blow and `fold_gear` keeps the stats current, so nothing is copied onto the wearer.

`corsair` uses all of it.

## Quests

`FactsPlugin` records facts with a kind, a subject, an object and an amount.
`Quests` are objectives over those facts, with prerequisite chains and a victory flag; `Counters` is a ledger of named tallies.
The grown-up version of [chapter 7](07-down-the-stairs.md)'s `if the king died`.

## Saving

`rl-save` has backends for files, memory and browser storage behind one resource, a versioned envelope that refuses a mismatch rather than guessing, and entity remapping.
The engine exports the scheduler's queue, the world's edits and places, and what has been explored.

## A world above the dungeon

`rl-world` generates one: FBM noise, elevation banding, priority-flood hydrology, climate, scored site placement and a road router.
`StreamingPlugin` streams it around the player with a seam hash so chunk edges agree, keeping your edits across unload and reload.
`rl-overworld` draws it as a map screen with a portal picker.

## Balance

`rl-rules::balance` scores threat and prints a band report over the `BandedTable` from [chapter 8](08-content-in-files.md).

```sh
cargo run -p corsair -- --balance
```

## The examples

| Example | What it shows |
|---|---|
| `delve` | Five floors of a beached whale, no surface at all; lighting, stealth and five knacks; `floors.rs` is the whole map builder |
| `corsair` | An open-world pirate roguelike with a pirate's abilities, built only on the public API |

## Reading further

- `docs/OVERVIEW.md`, the inventory of what exists and what does not.
- `docs/PLAN.md`, why, decision by decision.
- `docs/reviews/`, the reviews of the three codebases the engine was extracted from.
- `AGENTS.md`, the rules the build enforces and the rules review enforces.
