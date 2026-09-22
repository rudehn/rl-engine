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
  No pack or leader behaviour, no keep-at-range for a shooter, no patrol or idle routine, and no scent, though `DijkstraMap` is the right tool for it; noise is built, in `docs/design/noise.md`.
  A mind now shoots what it wields when there is a clear shot to take, with `ShootAtRange`, and holds a distance with `Shadow` above it; no game fields a skirmisher yet.
  `UseAbility` scores a footprint at two for a hit and three against for harm, hardcoded in `crates/rl-rules/src/ai/tactics.rs`; make the weights fields.

## 3. Make what exists real

- **Every game's log lines said inside a turn go through `Tell`.**
  Corsair, the tutorial, Delve and Heist still push lines straight to `MessageLog` from systems in `TurnSet::React` (the tutorial's "You eat the crust. It helps.", Corsair's portal and discovery lines, the heist's), against the narrator's module doc, so a line can land above the event it answers.
  Each should write a `Tell` instead, and the guide chapters that quote the tutorial move with it; Foundry did this on 2026-09-18.
- **A held key does not skip a cue.**
  `skip_on_key` in `crates/rl-render/src/particles.rs` skips only on a key just pressed, and a key held to repeat is read by `Repeats`, not `just_pressed`, so walking with a key held waits out every cue in sight.
  Measured on 2026-09-18 in Foundry: two seconds holding a direction key walked 15 steps on an empty lane and 4 beside a probe that pulses on each of its turns, with the turns held for 143 of the frames.
  A repeat that fires while the turns are held should skip the way a press does.
- **Straight-line fallbacks can cut a corner the move resolver refuses.**
  `Hunt`, `SearchLastKnown`, `Follow`, `FleeWhenHurt`, `GiveWay` and `Wander` try diagonal steps checked only by `can_step`, and `corner_ok` in `crates/rl-bevy/src/turn.rs` refuses a diagonal between two unwalkable cells, so a mind can spend turn after turn on a step that never happens.
  `Shadow` checks it with `squeezes`; the others should too, with the fingerprints that move re-baselined.

- **Movement profiles that change costs.**
  `FlowFields::ensure` keys the cache by `MovementProfile` but builds every map with `PathRules::default()`, so a swimmer and a walker see the same map and the sailing profile the plan's river section promised is not wired.
  `TileProps` needs a per-profile walkability mask, and the flood needs to read it.
- **Anyone travels.**
  `WarpRequest` and `GoThrough` ignore everyone but the player (`crates/rl-bevy/src/places.rs`, `resolve_warps`).
  Companions, escorts and a monster fleeing down the stairs are out of reach until a non-player can change maps.
- **A place for a miss.**
  Accuracy is deliberately absent (`docs/design/abilities.md`, "Accuracy does not exist"); the combat docs should say how a game adds a miss as a `DamageStage`, with an example.
- **A shot is narrated as a blow.**
  The phrasebook has one `HitsYou` for a blow and a shot alike, so a droid firing from across a dark room reads as "The line droid hits you for 4", named even when the player cannot see it.
  `DamageEvent` or `Struck` already knows whether an attack was ranged; a `ShootsYou` phrase, and "something" for an attacker out of sight, would say what happened.
- **Light is recast once a frame, not once a turn.**
  `update_lighting` runs in `EngineSet::Light`, after every `Turn` pass the frame ran, so a droid acting in the same frame the player switches a lamp off still sees by the old light, for one turn.
  Foundry's lamp shows it; recasting the dynamic layer inside the turn loop, when a source was added or removed, would close it.
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
- **Corsair's tests play a different game from its binary.**
  Corsair's binary runs `honour_portals`, `populate_places`, `drop_loot` and `inflict_on_hit`, and its test harness adds none of the four, so no Corsair test exercises them.
  One plugin that both the binary and the harness add, as Foundry's `FoundryPlugin` is, would close the gap for good.
- **Corsair, Delve and Heist have no map fingerprint tests.**
  A change to their maps goes unnoticed: the Foundry branch changed Corsair's cave maps and Delve's heart floor, and nothing in either game noticed.
  A fingerprint tripwire per game over a few seeds' maps, labelled as such, would make the next such change a deliberate re-baseline.

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
- **Admission scans its waiting actors linearly.**
  `admit_new_actors` (`crates/rl-bevy/src/turn.rs`) checks each waiting actor against the fresh list and the `arriving` list with a linear scan, so admission is quadratic in the number waiting, and it runs every pass.
  Harmless while only the player ever waits, and briefly; a game that parks a crowd on maps nobody has visited would pay for it.
  A `BTreeSet` of what has been seen makes it linear.
- **One allowlist entry in Foundry's ambiguity test is wider than its reason.**
  The entry on `Acting` and the action messages (`examples/foundry/src/plugin/ambiguity.rs`) admits any pair of systems, though its reason only holds for resolvers and sweepers.
  Narrow it to systems in `TurnSet::Resolve` and `TurnSet::Sweep`, so a future system that writes those outside them fails the test.

## 5. Documentation

The plugin table landed on 2026-09-21, with `scripts/check-overview.sh` behind it; the plan's progress log says what the review that prompted it found.
The design docs still owed, and which files a slice owes, are in `AGENTS.md`.

- **Guide chapters for the second half.**
  Lighting, stealth, abilities, statuses, saving and streaming each get one paragraph in `docs/guide/src/09-where-to-go-next.md`, and the alternative is the 1,280-line `examples/delve/src/main.rs`.
  Four chapters in the guide's style: lights out, being noticed, an ability in RON, saving the run.
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
  77 commits touched crate sources in the 30 days to 2026-09-17, changing about 2,100 lines of public declarations; `CHANGELOG.md` records them, but a release every few days is not a kindness to anyone depending on it.
  Publish the five Bevy-free crates first, since their APIs are the most settled and the most reusable on their own, and keep the Bevy layer on a git dependency until it stops moving.

## Tracked elsewhere

- The plan's "Next" line: nights on Corsair's surface, scripted encounters, Bevy UI presenters over the panel views, and the living-world-rogue conversion.
- `docs/OVERVIEW.md`, "Not built yet": cursed items, heat and cold, liquids and wind, lit detection ranges, mouse-to-tile, instanced terminal rendering.
