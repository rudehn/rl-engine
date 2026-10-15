# Outstanding work

What the engine should do next, and why.
`docs/PLAN.md` holds the decisions and the history, `docs/OVERVIEW.md` the inventory and its "Not built yet" list; this file holds the work that has been found and not yet started.
An item leaves this file in the commit that finishes it, with its reasoning moved to the plan's progress log.

Opened 2026-09-15 from an architecture review of `main` at `cf5f0a9`, and added to on 2026-09-22 from a second review of `main` at `c55a7e9`.
The first review's standing verdict: the structure is right, and the debt is behaviour the engine ships as data and every game rewrites on top of it, which is the failure `docs/PLAN.md` section 1 was written against.
The second review's: the contributor pattern the minds use is the engine's best seam and was never generalised, so the two other cross-cutting concerns, the save and the narrator, are closed lists every new subsystem has to go and edit; and the loops above tier 1 have no benches, so what they cost is argued rather than measured.

That review first filed what it read as a save data-loss bug, over `Fuel`, `LightSource`, `Aware` and `Heard`.
It is not one.
`docs/design/lighting.md` section 5 says fuel and a light source on an item are the game's to save with its item state, as `Enchant` already is, and `docs/design/noise.md` says `Heard` and `Aware` are lost on load on purpose, with the note that should either become worth saving the two go into `EngineSave` together.
The item is struck, and the one line that was left of it, a `Burning` entity coming back unlit, was fixed on 2026-09-30 by saving it in `EntityState`.
The reading it rested on is worth keeping as a caution: the save's coverage is legible only from three design docs, and nothing in `crates/rl-save/` states the rule it follows.

The sections below are thematic.
"The order" is the order to work in, and every open item is in it.

## The order

Ranked by impact against effort.
Impact is what a game or a player loses while it is unfixed; effort is the size of the change, including the tests and the re-baselines it drags with it.
Everything in the first band is either a bug, or cheap enough that the reasoning costs more than the work.

| # | Item | Section | Impact | Effort |
|---|------|---------|--------|--------|
| 1 | `OnMap` as a required component | 4 | medium | medium |
| 2 | What the save holds is stated where the save is | 7 | medium | medium |
| 3 | `OverworldPlugin` declares one requirement and needs four | 3 | medium | low |
| 4 | A searcher dithers where it lost the trail | 2 | medium | low |
| 5 | What a kind that travels still cannot do | 3 | medium | medium |
| 6 | Movement profiles that change costs | 3 | medium | high |
| 7 | The narrator hears what registers itself | 7 | medium | high |
| 8 | `Thinking` splits its context from its snapshot | 4 | low | low |
| 9 | `TargetView` holds the enum it keeps reconstructing | 4 | low | low |
| 10 | `WorldMap::tile` walks a `BTreeMap` per call | 8 | low | low |
| 11 | Admission scans its waiting actors linearly | 4 | low | low |
| 12 | One allowlist entry in Foundry's ambiguity test | 4 | low | low |
| 13 | `Rooms` can run out of attempts on a small map | 3 | low | low |
| 14 | The `Doors` pass hangs two doors back to back | 3 | low | low |
| 15 | `HalveIfBlocked` can never fire | 3 | low | low |
| 16 | Two engine types are named for a theme word | 4 | low | low |
| 17 | A light that moved costs a second cast, unmeasured | 8 | low | low |
| 18 | Split `crates/rl-bevy/src/ability.rs` | 4 | low | medium |
| 19 | Tactics that are missing, and weights that are fixed | 2 | medium | medium |
| 20 | The resolvers in `ResolveSet::Act` are unordered | 4 | low | medium |
| 21 | A prop cannot block sight, or block the way and let a shot past | 3 | medium | medium |
| 22 | `offer_abilities` declares `&mut Stack` it only reads | 4 | low | low |
| 23 | A piece stamped where no wall rings it can cut the floor | 1 | low | medium |
| 24 | Entity order still breaks ties in four places | 4 | low | low |
| 25 | Undermere's ordering check misses what only its binary adds | 3 | low | low |
| 26 | The pages still name the eight folded plugins | 5 | low | low |
| 27 | `UseCarried` decides by default when a mind mends | 4 | low | low |
| - | Everything in 5 and 6 | 5, 6 | gated | gated |

The first eight items of the order this file opened with were built on 2026-09-22, and the plan's progress log says how.
The corner-cutting fallbacks went the same day: every tactic that picks a neighbour itself now asks whether the move resolver would take that step.
The one that mattered most was the bench, which disproved the item that had been ranked first on the performance side: the turn loop is linear in the crowd, not quadratic, and the perceive stage's scans are not where the time goes.
Six more were built on 2026-09-30, healing at their head: healing as its own path to health, the light recast inside the turn, the flow field cache, every game's lines inside a turn through `Tell`, anyone travelling when its wits say so, and a `Burning` thing saved.
The table was renumbered that day, again on 2026-10-04 for two gaps Undermere's floors found, again on 2026-10-10 when the three furnishing items were built, again on 2026-10-11 when the coloured light and the player's long action were built, and again on 2026-10-12 when the two items that were only about the Corsair, Delve and Heist examples went with the games, so a number here is a place in the order as it stands and not the one an older note may cite.

Why the order that is left, in four moves:

1. **The middle band first, 1 to 5.**
   This is behaviour and consistency debt: what a second game hits, not a first.
   None of it is speculative, and none of it needs measuring first.
   Item 5 is travel: fielded now, and with three decisions left that a game taking it further will meet.
2. **Then 6 and 7**, the high-effort ones, of which only the narrator's registry is structural.
   Neither is urgent.
3. **Items 8 to 18 are cleanups** worth taking whenever their file is open for another reason rather than scheduling.
   Item 17 is a measurement, not a change: run the lit bench before deciding there is anything to fix.
   Item 22, added on 2026-10-09, is one more of these.
4. **The rest wait on something.**
   Item 19 waits on a game that actually wants the tactics it would add, and item 20 on a second game asking for it.
   Item 21 was raised on 2026-09-28 when Foundry's rooms were first furnished, and is what the furnished rooms most visibly lack; the three furnishing items raised beside it were built on 2026-10-10.
   Item 23 is what furnishing left on 2026-10-10, and waits on a game that stamps into a map `Accrete` did not grow.

Section 5 is documentation and section 6 is the release, and both are gated on the API settling rather than on this list.
Section 9 is low priority and deliberately outside the order.
The performance section is done for now: eight items opened there, the benches closed or struck seven, and what is left is one cheap cleanup and one cast to measure.

## 1. Own the loops the games keep rewriting

Its three original items and the swap below were built on 2026-09-16, and the plan's progress log says how.
Furnishing, the loop every game with rooms writes next, was opened on 2026-09-28 when `StampEachRoom` gave Foundry a piece in every room, and built on 2026-10-10 for Undermere's dungeon in two halves.
A pool, `rl_rules::pool` with `StampPicks` and `rl_bevy::layout::picked`, says which pieces a run holds, by floor, weight, limit, group and budget, and places each in the area it prefers.
A dressing, `rl_rules::dressing` with `Dress` and `rl_bevy::layout::dress`, fills every room the pool left from a weighted list by district, by a prefab's legend and rules for where each glyph goes rather than rows, so one dressing fills a closet, a hall and a cave, never beside an opening and never cutting an area in two.
Foundry still furnishes with `StampEachRoom`, and may adopt both.

- **A piece stamped where no wall rings it can cut the floor.** `Dress` floods an area before and after each thing, but a stamp into a cave on a map `Accrete` did not grow has no area to flood, and the `Placement::Fits` the prefab design proposes, anywhere a piece's cells land on open floor, wants the same check and rollback.

## 2. Open the minds

The five items that opened this section were built in the six stages of `docs/design/minds.md` on 2026-09-16; what remains is the tactics they make room for.

- **Tactics that are missing, and weights that are fixed.**
  No pack behaviour, no keep-at-range for a shooter, no patrol or idle routine, and no scent, though `DijkstraMap` is the right tool for it; noise is built, in `docs/design/noise.md`, and so is following a leader, in `docs/design/minds.md` section 7.
  A mind now shoots what it wields when there is a clear shot to take, with `ShootAtRange`, and holds a distance with `Shadow` above it; no game fields a skirmisher yet.
  `UseAbility` scores a footprint at two for a hit and three against for harm, hardcoded in `crates/rl-rules/src/ai/tactics.rs`; make the weights fields.
- **A searcher dithers where it lost the trail.**
  `SearchLastKnown` walks to the remembered cell and, standing on it, gives the turn to the next tactic, but the memory stays until `NoticeStats::memory` turns have passed.
  So whatever comes next steps off the cell and the search walks straight back: a posted guard shuffles between the cell and one step toward its `Post`, and a wanderer between the cell and a random neighbour, until the memory runs out.
  Seen on 2026-09-25 in Foundry's guard posts, where it reads as a guard that cannot decide to go home.
  The fix is to forget the remembered cell once the searcher has reached it and seen nothing there, in stealth's awareness update or in the tactic's own contract, so the search ends where it arrives and the next tactic takes over for good.
  It changes how every game's monsters search, so every fingerprint moves with it.

## 3. Make what exists real

- **Undermere's ordering check misses what only its binary adds.**
  `examples/undermere/src/plugin/ambiguity.rs` checks every system `UndermerePlugin` adds, on a headless app.
  The `ViewSet::Annotate` systems `main.rs`'s `add_panels` adds exist only in the binary, so a conflict among them, or between them and the plugin's, is never reported.
  Moving them into a plugin of their own that the test can add, or checking the binary's app too, would close it.
- **A prop cannot block sight, or block the way and let a shot past.**
  A prop's `blocks` is `Blocks`, which the occupancy index reads for movement and `combat::shot` reads as stopping a shot, and nothing a prop is can block sight.
  Tiles already split the three, walkable, opaque and whether a shot passes, and Foundry's rooms want the same of a prop: a coolant tank or a full shelf blocks all three, a workbench or a lathe only the way, as `examples/foundry/DESIGN.md` sets out under Rooms.
  Sight goes through the veil that gas writes into and a bump to the map's `opacity_epoch`, which `docs/design/props.md` section 10 already names; a low prop letting a shot over it wants `shot` to tell an actor standing in a cell from a thing that only blocks walking.

- **What a kind that travels still cannot do.**
  Travel is two wits since 2026-10-14, `Wits::FOLLOWS_THROUGH` and `Wits::FLEES_THROUGH`, and content's `travels` is both.
  Undermere fields both: its monkey's `stealing` writes `travels` to flee down the stairs with what it took, a freed captive is given both to follow the adventurer, and its mercenaries are taken off the fleeing half and sent after the adventurer by the game's own warps, which say whom they follow with `WarpRequest::after`.
  Three things want deciding when a game takes it further.
  Foundry's lifts are gated by the game for the commando, the core charged or not, and nothing asks that of a droid that travels.
  Following is only for a mind beside the one that left, as it left; a chase that arrives some turns later would need actors on a map nobody is reading to be dealt turns, which nothing does.
  And the narrator's `Leaves` cannot name the way, "leaves" rather than "takes the lift", because a transition has no name the engine reads.
- **The tutorial's own `narrate` pushes lines once a frame.**
  Steps 4 to 6 and 8 to 10 of the tutorial read `ItemEvent` in `PresentSet::Narrate` and push "You eat the crust. It helps." straight to the log, which is the pattern the narrator replaced: a frame of several passes says them in buffer order rather than in the order they happened.
  They are not inside a pass, so the rule about `Tell` does not reach them; moving them into `TurnSet::React` as a `Tell` would, and the guide chapters that quote them move with it.
- **Movement profiles that change costs.**
  `FlowFields::ensure` keys the cache by `MovementProfile` but builds every map with `PathRules::default()`, so a swimmer and a walker see the same map and the sailing profile the plan's river section promised is not wired.
  `TileProps` needs a per-profile walkability mask, and the flood needs to read it.
- **`Rooms` can run out of attempts on a small map.**
  Asked for three rooms sized 8 to 10 on a 40x30 map, it fails roughly one seed in sixty inside its default thirty attempts.
  Whether that is a tuning problem, a default `attempts` too low for the room sizes it is asked to fit, or a limit the pass should just document is not yet decided.
  Either way, a test that stamps rooms on a small map has to know this failure rate exists rather than treat every seed as good.
- **The `Doors` pass hangs two doors back to back.**
  It hangs a door on every cell of the ring round a room where a corridor meets it, and where two rooms sit two cells apart the two cells of corridor between them lie one on each room's ring, so each gets a door, one behind the other.
  Two doors stand where one was meant, and where they are shut a walker opens both to cross one gap.
  Seen in Undermere's halls and crypts, and Foundry's decks share it, since every game that chains `Rooms` and `Doors` does.
  The fix is to skip a ring cell whose neighbour across the gap is already a door, or to hang one door per corridor rather than one per room it meets; either moves the maps of every game that hangs doors, so their fingerprints are re-baselined with it.
- **`on_equip`.**
  Effects land once and wearing is a standing state, which is why `docs/design/items.md` section 6 defers it; a cursed plate that bites when put on is a real case and would be one more moment, reported by the items resolver on `Equip`.
  It waits for a game that wants it.

- **`HalveIfBlocked` can never fire.**
  The engine builds a `Defender` in exactly two places and both hardcode the flag: `apply_damage` at `crates/rl-bevy/src/combat.rs:659` and `expected_damage` at `crates/rl-rules/src/forecast.rs:70`, each `blocked: false`.
  Every other construction is a test in `crates/rl-rules/src/damage.rs`, so nothing outside the tests ever sets it true.
  `HalveIfBlocked` is a publicly exported `DamageStage` all the same, and a game that puts it in its `DamageStages` gets no block, no roll, no component to add and no warning that the stage is inert; it is reachable only by a caller driving `resolve` itself.
  `docs/guide/src/systems/combat.md` says that out loud and gives the workaround, rolling the block inside a stage of the game's own, which is correct and is why this is an engine gap rather than a documentation one.
  Either the stage goes, or `Defender` gains a way to be filled: a component `apply_damage` reads, or a seam that lets one stage set the flag for a later one.
  Found on 2026-09-22 while writing that page.
- **`OverworldPlugin` declares one requirement and needs four.**
  It calls `needs::<OverworldLayout>` and nothing else (`crates/rl-overworld/src/lib.rs`), but `draw_overworld`'s `Whereabouts` takes `Res<WorldRes>`, `Res<WorldMap>` and `Res<Knowledge>` without an `Option` between them, and `handle_keys` takes `Res<Knowledge>` and `ResMut<Modals>` the same way.
  A game that adds the screen without `StreamingPlugin`, and so without a `WorldRes`, gets a system Bevy skips rather than the combined, loud report at the start of play that `AGENTS.md` promises and that `check_requirements` exists to give.
  Three `needs::<_>` calls with hints, `WorldRes` naming `StreamingPlugin` as where one comes from, would put the screen back under the house rule.
  The same gap read from the other side is the module doc at `crates/rl-overworld/src/lib.rs:4-5`, which says the screen "reads the [`WorldRes`] and [`Knowledge`] and writes a [`PortalRequest`]" and leaves out `WorldMap`, the player's `Position` and `Modals`; it should list what the systems actually take once the declarations do.
  Found on 2026-09-22 while writing `docs/guide/src/systems/overworld.md`.
- **Two engine types are named for a theme word.**
  `crates/rl-ui/src/narrate.rs:376` declares `type Weapons<'w, 's>`, the query for items that strike or shoot when wielded, which the collector reads to tell a wield from a wearing; `crates/rl-bevy/src/combat.rs:510` declares `struct Weapon`, the attack one blow is made with as `Loadout` chose it.
  `crates/rl-ui/src/lib.rs` states the rule the first one breaks two files away in the same crate, under "Rules": "No engine type, doc or constant says weapon, spell or monster", and `AGENTS.md` forbids the vocabulary outright.
  Both are private and both behave correctly, so nothing is wrong at runtime; what is wrong is that the crate that states the rule is a crate that breaks it, and a reader who meets the type before the rule learns the wrong lesson.
  A sweep of `crates/` for declarations found these two and nothing else outside test modules, so it is two renames: the engine's own words are to hand, since the phrases either side of the narrator's call site are `YouWield` and `YouWear` and the local it fills is already `wielded`, and combat's struct is what `Loadout` returns.
  The same words do appear in doc comments across several crates, as illustration of what the engine refuses to model. That is settled practice rather than part of this item, and treating it as part of it would make the item the whole codebase.
  Found on 2026-09-22 while writing `docs/guide/src/systems/narration.md`.

## 4. Simplify

- **`UseCarried` decides by default when a mind mends.**
  `UseCarried::default()` in `crates/rl-rules/src/ai/tactics.rs` drinks to mend at half health, a rule number in the engine, which the rule that a game's numbers stay the game's (set 2026-10-13, when the standard chain was built) forbids.
  The other tactics' defaults are "whenever it can" or structural and are fine; this one wants no default, or a game's own threshold passed as the standard chain's numbers are, and Undermere's mercenaries and monsters writing theirs.
- **Entity order still breaks ties in four places.**
  The fingerprints and the engine's turn admission, noticing and noise stopped reading Bevy's entity numbers as spawn order on 2026-10-13, since any unrelated spawn renumbers what follows it.
  Four places still use the number as a tiebreak: Undermere's raising of the dead (`examples/undermere/src/effects/bodies.rs`), the order of a mercenary's "stays behind" lines (`examples/undermere/src/mercenaries/travel.rs`), a name pick in `crates/rl-ui/src/view/mod.rs`, and the engine's `Aware`, keyed by `Entity` in a `BTreeMap` whose iteration order anything walking it inherits.
  None moved a fingerprint in the proof runs; each wants a tie broken by position or by value, as the rest now are.
- **`Thinking` splits its context from its snapshot.**
  `crates/rl-bevy/src/minds.rs` keeps the read-only context, `at`, `reach` and `origin`, in the same resource as the snapshot being filled, so every contributor builds an intermediate `Vec` and `extend`s it at the end purely to satisfy the borrow checker.
  Splitting the two deletes that pattern from six call sites in five crates' worth of subsystems.
- **Split `crates/rl-bevy/src/ability.rs`.**
  At 1,562 lines it holds the state components, the effect registry, the gate, payment, `Offered`, the `Known` refresh, airborne landings and cue emission.
  State, registry and resolver submodules, and named `SystemParam`s in place of the four-tuple aliases `Spender` and `Bearing` that `gate` and `pay` destructure by position.
- **`TargetView` holds the enum it keeps reconstructing.**
  `ability`, `throwing` and `firing` are three fields for one sum type, and `Pointing::of` rebuilds it every frame (`crates/rl-ui/src/view/target.rs`).
  Store `Option<Pointing>`, and open `Pointing` so a game can aim something of its own through the shared cursor: a direction to dig, someone to talk to.
- **`OnMap` as a required component.**
  `on.map(|m| m.0).unwrap_or(MapId::SURFACE)` is written in turn, items, minds, places, status and both games, and tutorial step 1 has to explain why a delve's first floor is map one.
  Require `OnMap` on `Position` and the `Option` disappears everywhere.

- **Admission scans its waiting actors linearly.**
  `admit_new_actors` (`crates/rl-bevy/src/turn.rs`) checks each waiting actor against the fresh list and the `arriving` list with a linear scan, so admission is quadratic in the number waiting, and it runs every pass.
  Harmless while only the player ever waits, and briefly; a game that parks a crowd on maps nobody has visited would pay for it.
  A `BTreeSet` of what has been seen makes it linear.
- **One allowlist entry in Foundry's ambiguity test is wider than its reason.**
  The entry on `Acting` and the action messages (`examples/foundry/src/plugin/ambiguity.rs`) admits any pair of systems, though its reason only holds for resolvers and sweepers.
  Narrow it to systems in `TurnSet::Resolve` and `TurnSet::Sweep`, so a future system that writes those outside them fails the test.
- **`offer_abilities` declares `&mut Stack` it only reads.**
  The ability gate takes the resolver's `UserState` (`crates/rl-bevy/src/ability.rs`), whose `tagged` query is `Option<&mut Stack>` because paying an ability's cost spends from a stack, so the gate in `DecideSet::Offer` claims write access it never uses.
  Since a price on an offer is counted from the taker's stacks, `offer_here` reads `Stack` beside it, and the pair is declared `ambiguous_with` in `PropsPlugin` to state that neither writes; without it every game's ambiguity test would report them.
  Give the gate a read-only param of its own, and the `ambiguous_with` goes.

- **The resolvers in `ResolveSet::Act` are unordered, and every game's ambiguity test pays for it.**
  Nine systems resolve in that set and nearly all of them write `DamageEvent`, `Position`, `Stack`, `Occupancy` and the cue queue, with nothing declaring an order between them.
  It is safe: `Resolution::claim` spends one actor's one turn once a pass, so in the pass one resolver did something every other resolver found nobody to resolve for, and their relative order is unobservable.
  What it costs is that the safety has to be restated per pair and per game: twenty of the forty entries in `examples/foundry/src/plugin/ambiguity.rs` say only that, the count is quadratic in resolvers, and the second game to grow the same test pays it again from scratch.
  Note that the engine already ordered the part of that set where order *is* observable: `LandSet` chains abilities, throws and shots, because several landings can land in one pass and the first hit to take a target to nothing is credited with the kill.
  The fix is the same shape: an `ActSet` in `CorePlugin`, chained, one slot per resolver family and a `Game` slot at the end, as `DecideSet` already does, with each plugin putting its resolver in its own slot.
  It costs no parallelism now that the `Turn` schedule is single-threaded, and the order between mutually exclusive resolvers is arbitrary, which `LandSet`'s own doc already concedes for landings.
  What it loses is a forcing function: today a new resolver fails Foundry's test and somebody has to write down why it is safe, which is how `consumable::land_uses` was audited the day it was added; under a chain it slots in silently, and a resolver that quietly does not claim gets no prompt.
  Worth doing when either a resolver appears that genuinely can co-occur with another in one pass, or a second game grows an ambiguity test; not worth doing for tidiness alone.
  Rejected while writing this down: declaring the resolvers `ambiguous_with` each other once in the engine. It would silence every game's entries without inventing an order, but it suppresses rather than states, and it would hide the pair that one day really does conflict.

## 5. Documentation

- **The pages still name the eight folded plugins.**
  `ConsumablesPlugin`, `StatusPlugin`, `WorkPlugin`, `UnloadPlugin`, `SettingsSavePlugin`, `RecordsSavePlugin`, `PlayerSaves` and `InteractKey` became part of `ItemsPlugin`, `CorePlugin`, `SavePlugin` and `UiPlugin` on 2026-10-13.
  The overview and the guide's quotes were updated with them; the system pages' manifests and prose, the design docs and `docs/PLAN.md` were left for the release's documentation pass, and `scripts/check-systems.py` will flag the pages.
The plugin table landed on 2026-09-21, with `scripts/check-overview.sh` behind it; the plan's progress log says what the review that prompted it found.
The design docs still owed, and which files a slice owes, are in `AGENTS.md`.

- **Guide chapters for the second half.**
  Lighting, stealth, abilities, statuses, saving and streaming each get one paragraph in `docs/guide/src/09-where-to-go-next.md`, and the alternative is reading Undermere or Foundry whole.
  Four chapters in the guide's style: lights out, being noticed, an ability in RON, saving the run.
- **One picture of the frame.**
  `EngineSet`, the `Turn` passes and their sets are described in prose in `crates/rl-bevy/src/plugin.rs`; a diagram on one page of the guide would replace what readers reverse-engineer today.
- **Doc comments at `turn.rs` density.**
  Many carry the history of how they came to be.
  That belongs in the plan's progress log; the comment says what and why-not.
- **A start helper.**
  A game begins with seven incantations: the plugin group, the seed, the tiles, their appearance, `WorldMap::new(tables)`, `PlaceRulesRes`, a warp, and then the state flip to `Playing`.
  One `start_in_place(player, map)` command could take the last two, and the tutorial's chapter 1 shrinks with it.

- **The design notes no longer describe the code.**
  Six were mined for the system reference on 2026-09-21 and 2026-09-22 and every one was wrong in a structural claim rather than a detail: `docs/design/minds.md`, `remains.md`, `fields.md`, `noise.md`, `stealth.md` and `props.md` each name a type, a field or a mechanism that has moved or never existed, and two contradict themselves between a section and their own account of what the build changed.
  The pages in `docs/guide/src/systems/` are the accurate description now, and each one's manifest is checked against the files it documents by `scripts/check-systems.py`, which is the thing a note has no equivalent of.
  What a note is still right about is why a subsystem is shaped as it is; the fix is a line at the top of each saying the reference supersedes its model, and the decision about deleting them is `docs/PLAN.md`'s.
- **The worked example teaches an XOR where the rule says `derive`.**
  `docs/guide/src/systems/mapgen.md` spends four sentences of `The line` on a pass's stream coming from `RunSeed::derive`, which `AGENTS.md` names as the mechanism, and the snippet directly above it derives a floor's seed with `RunSeed(self.seed.0 ^ (depth as u64) << 32)` at `examples/tutorial/src/bin/step06_descent.rs:135`.
  It is still derived from the run's seed, so it does not break the rule about constants or entropy, and the choice of index is the caller's; what is wrong is that the example a reader copies is not the mechanism the page and the guide both name.
  Changing the line changes every map those seeds generate and may disturb fingerprint tests, so it is its own slice rather than a correction to the page.
  Recorded in the same breath: A*'s insertion-order tie-break was documented at `crates/rl-grid/src/astar.rs:10` with no test behind it until 2026-09-22, when writing `systems/grids.md` turned up the claim that ties are pinned by tests and only the flood half was.
  Other documented properties may be unpinned the same way, and a page that claims one is the occasion to check.

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

## 7. Open the seams the subsystems have to reach through

The minds' perceive stage is the engine's best seam: `crates/rl-bevy/src/minds.rs` says "a subsystem added later adds a contributor and edits nothing here", and it is true, with fire, stealth, items, props, abilities and noise each pushing in from their own module and `minds.rs` naming none of them.
The engine's two other cross-cutting concerns work the opposite way.
Each is a closed list in a crate the subsystem does not own, and each has to be edited by hand when anything new lands.
The run's teardown was a third, and stopped being one on 2026-09-22, when `ResetsOnNewRun` turned `clear_run`'s hand-written list into a registry; that is the shape the two below would take.

- **What the save holds is stated where the save is.**
  `EntityState` in `crates/rl-save/src/run.rs` is a fixed field list, and `EngineSave` in `engine.rs` has grown one `#[serde(default)]` per subsystem, so the natural reading is that a subsystem is saved when somebody remembered to add it.
  That reading is wrong, and the review that filed it fell for it: the engine draws a real line, per-instance state on content a *game* authors is the game's to save through `Saveable`, and the engine saves what it owns itself.
  The line is written down in `docs/design/lighting.md` section 5 and `docs/design/noise.md`, two crates away from the code that follows it, and nowhere in `crates/rl-save/`.
  The fix is a paragraph in `crates/rl-save/src/run.rs`'s module docs saying which side of the line a component falls on and why, and a line on `EntityState` saying it holds the engine's own state and not the game's.
  A registry that let each plugin declare its own capture, the shape the minds' perceive stage uses, is the larger version of this and is not obviously worth its cost: it would buy `rl-save` out of depending on six subsystems, and it would buy nothing else, since nothing is actually falling through today.
  Write the paragraph first and see whether the registry still looks necessary afterwards.
- **The narrator hears what registers itself.**
  `Heard` in `crates/rl-ui/src/narrate.rs` is a `SystemParam` holding twelve `MessageReader`s, one per engine subsystem, feeding a closed `Phrase` enum and a `Phrasebook` of defaults.
  Adding a subsystem to `rl-bevy` therefore means editing `rl-ui`: a reader, a variant, a template.
  The enum being closed is defensible, since it enumerates the events the engine itself raises, but the *reading* need not be: a `app.narrates::<LightEvent>(..)` that turns one message kind into rows would let each subsystem carry its own phrases and its own defaults.
- **A game cannot refuse an offer at the gate.**
  `Refused` in `crates/rl-bevy/src/props.rs` says only `Needs` and `Short`, so an offer the game would judge pointless, such as Undermere's `mend` for an adventurer already whole, is taken up, spends its turn and its price, and is answered with the price handed back by `pay`.
  The seam is a game-supplied refusal read at the offer gate in `DecideSet::Offer`, greyed on the offers screen like a short purse, costing neither the turn nor the coins; `Interacted` carrying the price the engine took would let a refunding game read what was paid rather than repeat its own table.

## 8. Make the loops cheap, and know that they are

Three benches now cover this section: `crates/rl-bevy/benches/turns.rs` on a player turn, `crates/rl-ui/benches/frame.rs` on a frame, and `crates/rl-bevy/benches/passes.rs` on where each of those goes.
Between them they have settled five items here and struck two, most of them against the reading that filed them.
What is left below is measured unless it says otherwise.

- **Closed: a lit frame's field of view is already about as cheap as it can be.**
  Investigated with counters on 2026-09-22, and the plan the item carried was wrong in both halves.
  There is no double cast: over one player turn with thirty-two minds, the count is exactly 1.00 shadowcasts per actor, all of them in the frame's pass, and `sense` fires zero times, whether the minds are stationary or hunting.
  So `sense` is not covering the minds the frame's pass would skip, and skipping them would move the cost rather than remove it: each mind still needs its sight at its own pass.
  And the cheaper-test idea does not pay either.
  A mind asks its viewshed thirty-two point-visibility questions per turn with thirty-two other actors about, one per candidate in the roster, so the 10.3-microsecond shadowcast is answering them at 0.32 microseconds each.
  Point line-of-sight would be about 0.1 to 0.2 each, so it wins at small crowds and loses by sixty-four, where the shadowcast is flat and the question count is not.
  The shadowcast amortises, which is the right structure.
  What is left is the 10 microseconds per actor per turn that a lit game pays for every mind having sight, and that is the price of the feature.
  The bounded invalidation added the same day was the only real slack, and it was 10 per cent.
- **Struck: an instanced terminal.**
  Counted on 2026-09-22 by `cell_churn` in `crates/rl-ui/benches/frame.rs`, over Foundry's hundred by forty screen, four thousand cells.
  A still frame changes nothing at all.
  A frame the player stepped in changes 56 cells unlit and 47 lit, about 1 per cent.
  A flickering torch is the only thing that churns: 293 cells a frame standing still, 8 per cent, and it moves with the animation clock rather than with the player.
  But every one of those 293 is a colour change and none of them is a new glyph, and `flush_terminal` only rewrites the `Text2d` string when the character changed; a colour writes `TextColor` and lays out no text.
  So the worst frame in a lit, flickering game is about fifty glyph rewrites and three hundred colour writes, not the four thousand text re-layouts the instancing argument assumed.
  The diffed renderer is already doing the work instancing would have done.
  This should come off `docs/OVERVIEW.md`'s "Not built yet" list, or stay there with this number beside it so nobody argues for it again from the entity count.
- **Closed: the veil's global epoch costs nothing.**
  Measured on 2026-09-22 by `one_player_turn_in_smoke` in `crates/rl-bevy/benches/turns.rs`, thirty-two actors: no gas plugin 315 microseconds, the plugin with clear air 328, thick gas that never veils 387, and thick smoke that does veil 373.
  The veiling case is the cheaper of the two thick ones, within noise of it, so the epoch bump is free and what gas costs is the diffusion itself, about 60 microseconds a turn.
  The reason is the item above: every viewshed is already recast every turn, because the player moves and carries a light, so an epoch bump that marks them all stale adds no casts to a frame that was going to do them anyway.
  It would only bite in a game whose viewsheds are otherwise still, and no game here is one.
  Left alone.
  Note for anyone measuring this again: the first run of this bench was taken with a load average of 22 and reported `clear_air` at 548 microseconds, which is nonsense; measured alone on a quiet machine it is 328.
  The diffusion itself was rewritten on 2026-09-23 to spill and swirl, and measured against the old step in one process, the two alternating so a busy machine weighs on both alike, with a tile lookup behind what holds gas as the game has.
  One grenade's cloud on an eighty by fifty field costs less than the old step did, about 55 microseconds against 80, because whether a cell holds gas is now asked once a cell rather than by each of its neighbours; a field full of gas costs about 1.65 times as much, 134 against 81.
  The bench above has not been rerun on a quiet machine since.
- **Struck: view collectors run for screens nobody opened.**
  Measured on 2026-09-22 by `crates/rl-ui/benches/frame.rs`, at thirty-two actors in sight: the map alone is 265 microseconds a frame, the rail a game always shows takes it to 300, and adding the four screen-backed views whose screens nobody has opened takes it to 309.
  Nine microseconds of a 309-microsecond frame, three per cent, against a change that breaks two of the five collectors it would gate: `open_on_crowded_bump` reads `OffersView` every frame to decide whether to open the offers screen, and the menu reads `SheetView` at the end of a run to write the obituary.
  Not worth it.
  `docs/design/ui.md`'s "every frame, not on a turn boundary" stands, and this is a second reason for it.
- **A light that moved costs a second cast, unmeasured.**
  Since 2026-09-30 the light is recast inside the turn loop, so a mind within reach of a light that moved is marked stale and casts its sight in `sense` on its own turn, and again in the frame's pass if it then stepped.
  That is up to two shadowcasts a turn for the minds round a player carrying a lamp, where the count was exactly one, at about 10 microseconds each by the figures above.
  Run `one_player_turn` lit before deciding anything: a cast that only the light invalidated needs the gate and not the line, which `Viewshed` already keeps, and that is the fix if the number is worth one.
- **`WorldMap::tile` walks a `BTreeMap` per call.**
  `active_place()` does a lookup on every `tile`, `is_walkable` and `is_opaque`, and `draw_map` asks two or three times per cell per frame.
  Caching the active place behind the switch would take thousands of lookups a frame down to none.

## 9. Low priority, and not soon

Left over from the item triggers slice on 2026-09-23, found by its review and judged not worth a change yet.
None of them is in the order above and none is scheduled: take one when its file is open for another reason, or when a game runs into it.

- **Refilling charges keep their clock in a `Local`.**
  `recharge_charges` in `crates/rl-bevy/src/consumable.rs` remembers the last clock reading it saw, so loading a save with a later clock in the same process after some play credits the whole gap and refills every wand at once.
  Nothing uses `recharge` yet; the reference reading belongs to the run, reset when one starts or is loaded.
- **An empty kept thing still lands its `land` trigger when thrown, and throwing a charged wand spends a charge.**
  Throwing reports `land` whatever the thing holds, and `land` spends.
  Whether an empty wand thrown should do anything, and whether throwing a wand should cost it a charge, are rulings to make when a game has a wand worth throwing.
- **An item with shared `effects` and no triggers is accepted silently.**
  `Triggers::build` checks each trigger and says nothing of an `effects` list no trigger delivers, which is as much a typo as a trigger with nothing to land.
- **A negative `Burst` radius is not refused.**
  It covers nothing, silently; the loaders should refuse it by name.
- **A user that does not block movement is not a target of its own `use`.**
  `land_triggers` finds targets through `Occupancy`, which indexes only what `Blocks`; the old `land_uses` named the user outright.
  Every actor that uses things today blocks, so nothing loses a mend.
- **The chain-of-barrels test does not assert the two bursts land a pass apart.**
  `a_chain_of_barrels_goes_off_one_after_another_and_each_once` counts two reports; the design guarantees the pass between them and the test would not notice it go.
- **No test throws a thing that has both a `use` and a `land` trigger.**
  Throwing only ever reports `land`, so the gap is small; a potion that is drunk or thrown is the case to pin.
- **No test that merging a stack keeps the receiving stack's `left`.**
  The spec's testing section asks for one.
- **The gear panel shows nothing for an empty single-charge kept thing, where the bag says it is empty.**
  `collect_gear` shows charges only for a thing that holds more than one.

Left over from the worn gear and unseen slice on 2026-09-27, the same way.

- **A use made where its user stands that harms others does not end the unseen.**
  `reveal_attackers` in `crates/rl-bevy/src/stealth.rs` ends it on a blow or shot, a throw, and anything aimed, an ability or a thing's aimed use, that came down on another, read off `LandedOn` since 2026-10-10; a thing used where its user stands whose `use` trigger harms those around them is not on it, and `wake_on_damage` wakes nobody to an attacker still unseen.
  A player who sets something off on a crowd while hidden would expect to be seen, and the rule would read the landing of such a `use` trigger the way it reads an aimed one's, which `land_triggers` would report as `LandedOn` too.

Left over from the layout slice on 2026-10-01: the three pieces of work `docs/design/layout.md` section 0 says key on what it publishes.
The furnisher and the run director were built on 2026-10-10 as the dressing and the pool, and section 1 says how; what is left is:

- **Tells.**
  A thing seen before it is reached.
- **Foundry's recipes are a first draw.**
  Two recipes over three districts at 84 by 48; the size and the mix are meant to grow as the furnisher and the director give a deck more to hold.

Left over from Undermere's dungeon part on 2026-10-11, found by its slices' reviews and not done by the sweep that followed.
None is scheduled: take one when its file is open.

- **A higher `fill_pct` making a smaller cave is unpinned.**
  `Cave`'s docs say more rock to start means a smaller cave, and no test holds it, so a change to the automaton could reverse the claim unseen.
  `crates/rl-mapgen/src/cave.rs`.
- **A lake is measured before it is trimmed, and has no largest share.**
  `Lakes` counts a lake's size as drawn, then takes rock only where every way beside it goes under too, so a lake may end smaller than its range says, and nothing stops one covering most of an area.
  Its barred landings, the flank rule for a bridge and the `Stamped` ring are each untested.
  `crates/rl-mapgen/src/lakes.rs`.
- **A non-cut pond can take the one-thick wall between two ponds,** leaving a window between two areas that were separate.
  `Crossing::flanked` is not measured again after a later lake changes the banks.
- **An inverted range panics.**
  `Lakes` and `Bridges` take ranges as `(fewest, most)` with `most` below `fewest` unchecked, so a game's loader must check them itself.
  The same is true of `SpotRules` with `far` below `near`, which only a debug build asserts.
- **A forced wait counts as stillness.**
  `PropsPlugin`'s spotting roll multiplies the chance on a turn the player spent waiting, and cannot tell a wait the player chose from one a status forced, so being held searches.
- **A painted piece's unjoined entry shows the map's fill.**
  An entry a piece did not join stays the tile the map was before, which in a game whose fill is not its wall reads as a different wall in the piece's side; Undermere repaints it with a pass of its own, `PieceLook`.
- **An `Inside` pick that fits nowhere wears out retries.**
  `StampPicks` fails a place for a required pick no area holds and `run_retrying` grows it again, each attempt as hopeless as the last when the recipe has no area of the shape; refusing such an entry at load would cost nothing.
- **A game's log line always lands after the engine's lines of the same pass.**
  The narrator reads a game's `Tell` after the pass's own events, so a game cannot say a cause before the effect the engine narrates: Undermere words a trap's line in the past tense, "You stepped on a hidden dart plate!", to read right under "The dart plate hits you for 4.".
  Found while photographing Undermere's traps.

## Tracked elsewhere

- The plan's "Next" line: nights on Corsair's surface, scripted encounters, Bevy UI presenters over the panel views, and the living-world-rogue conversion.
- `docs/OVERVIEW.md`, "Not built yet": heat and cold, liquids and wind, lit detection ranges, hover and click-to-travel over the pointer, instanced terminal rendering.
