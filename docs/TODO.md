# Outstanding work

What the engine should do next, and why.
`docs/PLAN.md` holds the decisions and the history, `docs/OVERVIEW.md` the inventory and its "Not built yet" list; this file holds the work that has been found and not yet started.
An item leaves this file in the commit that finishes it, with its reasoning moved to the plan's progress log.

Written 2026-09-15 from an architecture review of `main` at `cf5f0a9`.
The review's standing verdict: the structure is right, and the debt is behaviour the engine ships as data and every game rewrites on top of it, which is the failure `docs/PLAN.md` section 1 was written against.

The order below is the recommended order.
The first section is what every game hits in its first week.

## 1. Own the loops the games keep rewriting

Nothing is left in this section; its three items and the swap below were built on 2026-09-16, and the plan's progress log says how.

## 2. Open the minds

The five items that opened this section were built in the six stages of `docs/design/minds.md` on 2026-09-16; what remains is the tactics they make room for.

- **Tactics that are missing, and weights that are fixed.**
  No pack or leader behaviour, no keep-at-range for a shooter, no patrol or idle routine, no noise or scent, though `DijkstraMap` is the right tool for the last two.
  `UseAbility` scores a footprint at two for a hit and three against for harm, hardcoded in `crates/rl-rules/src/ai/tactics.rs`; make the weights fields.

## 3. Make what exists real

- **Movement profiles that change costs.**
  `FlowFields::ensure` keys the cache by `MovementProfile` but builds every map with `PathRules::default()`, so a swimmer and a walker see the same map and the sailing profile the plan's river section promised is not wired.
  `TileProps` needs a per-profile walkability mask, and the flood needs to read it.
- **Anyone travels.**
  `WarpRequest` and `GoThrough` ignore everyone but the player (`crates/rl-bevy/src/places.rs`, `resolve_warps`).
  Companions, escorts and a monster fleeing down the stairs are out of reach until a non-player can change maps.
- **A place for a miss.**
  Accuracy is deliberately absent (`docs/design/abilities.md`, "Accuracy does not exist"); the combat docs should say how a game adds a miss as a `DamageStage`, with an example.
- **A ranged fighter is under-forecast.**
  `rl_rules::forecast::Combatant::strikes` is filled from `Loadout::blows`, the melee roll plus extra strikes; a `RangedAttack`'s dice never enter the forecast, so a combatant that only shoots reads as unable to hurt anything.
  A fix needs the ranged roll and `RangedAttack::cost` fed into `Combatant` for whichever side of the pair is not adjacent to the other, so the forecast picks melee or ranged per pair instead of assuming melee always applies.
- **`DamageStages` must not default to empty.**
  A game that forgets it gets raw damage and no word about why, which is the "a resource happens to exist" pattern the rules ban.
  Default to `SubtractArmor`, or declare it with `needs`.
- **`Rooms` can run out of attempts on a small map.**
  Asked for three rooms sized 8 to 10 on a 40x30 map, it fails roughly one seed in sixty inside its default thirty attempts.
  Whether that is a tuning problem, a default `attempts` too low for the room sizes it is asked to fit, or a limit the pass should just document is not yet decided.
  Either way, a test that stamps rooms on a small map has to know this failure rate exists rather than treat every seed as good.

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
  Lighting, stealth, abilities, statuses, saving and streaming each get one paragraph in `docs/guide/src/09-where-to-go-next.md`, and the alternative is the 1,280-line `examples/delve/src/main.rs`.
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

## 6. Publish it

- **The name is taken.**
  `rl-core` is a token-bucket rate limiter on crates.io at 1.22.0, so the foundation crate cannot keep its name, and the `rl-` family cannot keep its prefix without one odd crate out.
  Checked as whole families, with the base name and `-core` and `-grid` all free: `roguelike`, `dungeoneer`, `torchlit`, `runedeep`, `vaults`, `morgue`; taken: `rogue`, `delver`, `warren`, `gloom`, `crawl`.
  The shape to copy is bracket-lib's, where one word is both the facade crate and the prefix for the parts.
  Renaming reaches 297 references across 153 files and the public path `rl_engine::rl_core::Rect` that the guide teaches, so it is a decision to make before the first release rather than after it.
  Reserve the rest of the family the same day; nothing stops someone taking `rl-grid` tomorrow.
- **What blocks `cargo publish` is version requirements, not metadata.**
  Every publishable crate already carries a licence, description, repository, homepage, five keywords, categories and an MSRV, the examples and the tutorial are `publish = false`, and both licence files sit at the root.
  What stops a publish is 51 path dependencies with no version requirement, and they all flow through one `[workspace.dependencies]` table, so it is eleven lines.
  Ten of the eleven crates have no readme, so their crates.io pages would render empty, and `rl-engine`'s `readme = "../../README.md"` points outside its own package, which `cargo package` refuses.
  Publish in tier order, waiting for the index between each, and dry-run every crate first; `cargo-release` or `release-plz` does the ordering and is worth adopting before the first release rather than after.
- **Nothing should go out while the API moves this fast.**
  77 commits touched crate sources in the 30 days to 2026-09-17, changing about 2,100 lines of public declarations, and there is no `CHANGELOG.md`.
  Publish the five Bevy-free crates first, since their APIs are the most settled and the most reusable on their own, and keep the Bevy layer on a git dependency until it stops moving.

## Tracked elsewhere

- The plan's "Next" line: nights on Corsair's surface, scripted encounters, Bevy UI presenters over the panel views, and the living-world-rogue conversion.
- `docs/OVERVIEW.md`, "Not built yet": cursed items, heat and cold, liquids and wind, lit detection ranges, mouse-to-tile, instanced terminal rendering.
