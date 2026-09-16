# Outstanding work

What the engine should do next, and why.
`docs/PLAN.md` holds the decisions and the history, `docs/OVERVIEW.md` the inventory and its "Not built yet" list; this file holds the work that has been found and not yet started.
An item leaves this file in the commit that finishes it, with its reasoning moved to the plan's progress log.

Written 2026-09-15 from an architecture review of `main` at `cf5f0a9`.
The review's standing verdict: the structure is right, and the debt is behaviour the engine ships as data and every game rewrites on top of it, which is the failure `docs/PLAN.md` section 1 was written against.

The order below is the recommended order.
The first section is what every game hits in its first week.

## 1. Own the loops the games keep rewriting

- **A default narrator.**
  Ten files across the two games, the tutorial and the template turn `DamageDealt` and `DeathEvent` into the same log lines (`grep -l DamageDealt examples templates`).
  The plan's section 3.2 lists a `Narrator` trait that was never built.
  A collector in `PresentSet::Narrate` that renders engine events through game-supplied templates keeps the rule that a view holds no string the game did not supply.
- **Bump to attack as an opt-in.**
  Walking into a hostile becomes an `Attack` in nine copies of the same match on `Occupancy::first_at`.
  By the facet rule, the second game that writes a thing is the signal it belongs in the engine; the ninth is overdue.
  A `CombatPlugin` option, or a `Step` resolver that consults `CombatRules` when the way is blocked by an actor.
- **Consumables through abilities.**
  `UseItem` is only an event, so Corsair's `use_items` hardcodes what rum does.
  Abilities already have costs, charges and effects.
  Let a carried item `Grants` an ability with `Cost::Charge`, and let `UseItem` resolve as `Use` of what the item grants, so a potion is a line of RON.
- **A save seam for the game's entities.**
  `EngineSave` captures the engine's state, and Corsair writes 546 lines (`examples/corsair/src/save.rs`) to capture and restore its own, by definition name and `SaveId`.
  Every game that saves writes this.
  A per-definition capture and restore registered beside the definition type, so the game says what a kind of thing is and the engine walks the world.
- **Tile appearance from RON.**
  Statuses, affixes and abilities load through `Names`, but every game builds `TileAppearance` in Rust with colour literals, which is the largest block of tutorial step 1.
  A loader beside the others, and chapter 8 gains the tiles.

## 2. Open the minds

- **A perceive set instead of one god-module.**
  `Sight`, `MindWorld` and `Belongings` in `crates/rl-bevy/src/minds.rs` enumerate abilities, items, throwing, lighting, stealth and fire, so every new subsystem edits `minds.rs`, and `Snapshot` is closed to a game's own knowledge.
  A `DecideSet::Perceive` set in which each plugin fills its part of the snapshot inverts the dependency and gives a game tactic a slot of its own in the same motion.
- **A typed decision for the game.**
  `Decision::Game(u32)` is a magic number answered by a match in `DecideSet::Game`, which is the `Custom { id }` shape one level down.
  Carry a typed payload, or let a game tactic write its own intent through the perceive set's seam.
- **Minds without combat.**
  `MindsPlugin` depends on `CombatPlugin`, and every perceivable actor must carry `Health` and `Faction` (`ActorData` in `minds.rs`), so a stealth-only or non-violent game cannot field a mind, and a prop or a civilian is invisible to one.
  Make the faction matrix and health optional inputs to the snapshot.
- **Sight that is not the player's.**
  `perceivable` uses the player's viewshed as the one line-of-sight oracle, so nothing perceives anything the player has no line to.
  Deliberate and documented, and the limit on faction wars and a living world; the conversion the plan's "Next" line names will need per-faction or per-actor sight.
- **Flow fields toward goals other than the player.**
  Section 3.7 of the plan promised maps toward items, exits and allies; `FlowFields` builds only toward the player, so a companion cannot be hunted and a fetch cannot be pathed.
  Key the fields by goal set as well as profile.
- **Tactics that are missing, and weights that are fixed.**
  No pack or leader behaviour, no keep-at-range for a shooter, no patrol or idle routine, no noise or scent, though `DijkstraMap` is the right tool for the last two.
  `UseAbility` scores a footprint at two for a hit and three against for harm, hardcoded in `crates/rl-rules/src/ai/tactics.rs`; make the weights fields.
- **A stream of the minds' own.**
  `decide_minds` seeds a per-turn generator from `CombatRng` and a position hash (`minds.rs`, `turn_rng`).
  Give minds a `Stream` like combat and abilities have, so adding a tactic cannot shift combat's rolls.
- **Dead field.**
  `Snapshot::came_from` (`crates/rl-rules/src/ai/snapshot.rs`) is never written and never read.
  Delete it, or fill it from the last step and let `Wander` stop doubling back.

## 3. Make what exists real

- **Movement profiles that change costs.**
  `FlowFields::ensure` keys the cache by `MovementProfile` but builds every map with `PathRules::default()`, so a swimmer and a walker see the same map and the sailing profile the plan's river section promised is not wired.
  `TileProps` needs a per-profile walkability mask, and the flood needs to read it.
- **Anyone travels.**
  `WarpRequest` and `GoThrough` ignore everyone but the player (`crates/rl-bevy/src/places.rs`, `resolve_warps`).
  Companions, escorts and a monster fleeing down the stairs are out of reach until a non-player can change maps.
- **Attack cost, and a place for a miss.**
  `resolve_attacks` charges `BASE_ACTION_COST` for every blow.
  Weapon speed is the most common combat knob; put the cost on `MeleeAttack` and `RangedAttack`.
  Accuracy is deliberately absent (`docs/design/abilities.md`, "Accuracy does not exist"); the combat docs should say how a game adds a miss as a `DamageStage`, with an example.
- **`DamageStages` must not default to empty.**
  A game that forgets it gets raw damage and no word about why, which is the "a resource happens to exist" pattern the rules ban.
  Default to `SubtractArmor`, or declare it with `needs`.

## 4. Simplify

- **Split `crates/rl-bevy/src/ability.rs`.**
  At 1,562 lines it holds the state components, the effect registry, the gate, payment, `Offered`, the `Known` refresh, airborne landings and cue emission.
  State, registry and resolver submodules, and named `SystemParam`s in place of the four-tuple aliases `Spender` and `Bearing` that `gate` and `pay` destructure by position.
- **`TargetView` holds the enum it keeps reconstructing.**
  `ability`, `throwing` and `firing` are three fields for one sum type, and `Pointing::of` rebuilds it every frame (`crates/rl-ui/src/view/target.rs`).
  Store `Option<Pointing>`, and open `Pointing` so a game can aim something of its own through the shared cursor: a direction to dig, someone to talk to.
- **`OnMap` as a required component.**
  `on.map(|m| m.0).unwrap_or(MapId::SURFACE)` is written in turn, items, minds, places, status and both games, and tutorial step 1 has to explain why a delve's first floor is map one.
  Require `OnMap` on `Position` and the `Option` disappears everywhere.
- **Streams out of the prelude.**
  Corsair's loot drop rolls from `ResMut<CombatRng>` (`examples/corsair/src/items.rs`, `drop_loot`), which the randomness rule forbids; a game's draws come from `Seed::stream`.
  Remove `CombatRng` and `AbilityRng` from the preludes, and fix the drop.

## 5. Documentation

- **Guide chapters for the second half.**
  Lighting, stealth, abilities, statuses, saving and streaming each get one paragraph in `docs/guide/src/12-where-to-go-next.md`, and the alternative is the 1,280-line `examples/delve/src/main.rs`.
  Four chapters in the guide's style: lights out, being noticed, an ability in RON, saving the run.
- **A plugin table in the overview.**
  The rl-bevy section of `docs/OVERVIEW.md` is bullets of a hundred to two hundred words each.
  A table of plugin, what it needs, what it adds and what it emits serves a returning reader; the prose stays for the why.
- **One picture of the frame.**
  `EngineSet`, the `Turn` passes and their sets are described in prose in `crates/rl-bevy/src/plugin.rs`; a diagram on one page of the guide would replace what readers reverse-engineer today.
- **Doc comments at `turn.rs` density.**
  Many carry the history of how they came to be.
  That belongs in the plan's progress log; the comment says what and why-not.
- **A start helper.**
  A game begins with seven incantations: the plugin group, the seed, the tiles, their appearance, `WorldMap::new(tables)`, `PlaceRulesRes`, a warp, and then the state flip to `Playing`.
  One `start_in_place(player, map)` command could take the last two, and the tutorial's chapter 1 shrinks with it.
- **Split the delve's `main.rs`** into input, narration and content modules the way Corsair is, so the second worked example reads at the same grain as the first.

## Tracked elsewhere

- The plan's "Next" line: nights on Corsair's surface, scripted encounters, Bevy UI presenters over the panel views, and the living-world-rogue conversion.
- `docs/OVERVIEW.md`, "Not built yet": cursed items, heat and cold, liquids and wind, lit detection ranges, mouse-to-tile, instanced terminal rendering.
