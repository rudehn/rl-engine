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
The item is struck, and what is left of it is the one line in section 3 about `Burning`.
The reading it rested on is worth keeping as a caution: the save's coverage is legible only from three design docs, and nothing in `crates/rl-save/` states the rule it follows.

The sections below are thematic.
"The order" is the order to work in, and every open item is in it.

## The order

Ranked by impact against effort.
Impact is what a game or a player loses while it is unfixed; effort is the size of the change, including the tests and the re-baselines it drags with it.
Everything in the first band is either a bug, or cheap enough that the reasoning costs more than the work.

| # | Item | Section | Impact | Effort |
|---|------|---------|--------|--------|
| 1 | `FlowFields` thrashes rather than evicts | 8 | medium | medium |
| 2 | Corsair's tests play a different game from its binary | 3 | medium | medium |
| 3 | No map fingerprint tests for Corsair, Delve and Heist | 3 | medium | low |
| 4 | `OnMap` as a required component | 4 | medium | medium |
| 5 | Light is recast once a frame, not once a turn | 3 | medium | medium |
| 6 | Every game's log lines go through `Tell` | 3 | medium | medium |
| 7 | What the save holds is stated where the save is | 7 | medium | medium |
| 8 | Anyone travels | 3 | medium | high |
| 9 | Movement profiles that change costs | 3 | medium | high |
| 10 | The narrator hears what registers itself | 7 | medium | high |
| 11 | `Thinking` splits its context from its snapshot | 4 | low | low |
| 12 | `TargetView` holds the enum it keeps reconstructing | 4 | low | low |
| 13 | `WorldMap::tile` walks a `BTreeMap` per call | 8 | low | low |
| 14 | Admission scans its waiting actors linearly | 4 | low | low |
| 15 | One allowlist entry in Foundry's ambiguity test | 4 | low | low |
| 16 | A `Burning` entity comes back unlit | 3 | low | low |
| 17 | `Rooms` can run out of attempts on a small map | 3 | low | low |
| 18 | A place for a miss | 3 | low | low |
| 19 | Split `crates/rl-bevy/src/ability.rs` | 4 | low | medium |
| 20 | Tactics that are missing, and weights that are fixed | 2 | medium | medium |
| 21 | The resolvers in `ResolveSet::Act` are unordered | 4 | low | medium |
| 22 | `HalveIfBlocked` can never fire | 3 | low | low |
| 23 | Two engine types are named for a theme word | 4 | low | low |
| 24 | `OverworldPlugin` declares one requirement and needs four | 3 | medium | low |
| - | Everything in 5 and 6 | 5, 6 | gated | gated |

The first eight items of the order this file opened with were built on 2026-09-22, and the plan's progress log says how.
The corner-cutting fallbacks went the same day: every tactic that picks a neighbour itself now asks whether the move resolver would take that step.
The one that mattered most was the bench, which disproved the item that had been ranked first on the performance side: the turn loop is linear in the crowd, not quadratic, and the perceive stage's scans are not where the time goes.

Why the order that is left, in three moves:

1. **The performance section is done for now.**
   Eight items opened there; the benches closed or struck seven of them and one line fixed the eighth.
   The turn loop's ceiling was schedule dispatch, and everything else that was supposed to be a ceiling measured small: the perceive scans, the veil's epoch, the closed screens' collectors, the terminal's entity count.
   What is left in section 8 is one cache that thrashes and two cheap cleanups, none of them urgent.
2. **So start at item 1 and work down the middle band, 1 to 8.**
   This is behaviour and consistency debt: what a second game hits, not a first.
   None of it is speculative, and none of it needs measuring first.
3. **Then 9 to 11**, the high-effort ones, of which only the narrator's registry is structural.
   Neither is urgent.

Items 12 to 19 are cleanups worth taking whenever their file is open for another reason rather than scheduling, item 20 waits on a game that actually wants the tactics it would add, and item 21 waits on a second game asking for it.
Section 5 is documentation and section 6 is the release, and both are gated on the API settling rather than on this list.
Section 9 is low priority and deliberately outside the order.

## 1. Own the loops the games keep rewriting

Nothing is left in this section; its three items and the swap below were built on 2026-09-16, and the plan's progress log says how.

## 2. Open the minds

The five items that opened this section were built in the six stages of `docs/design/minds.md` on 2026-09-16; what remains is the tactics they make room for.

- **Tactics that are missing, and weights that are fixed.**
  No pack or leader behaviour, no keep-at-range for a shooter, no patrol or idle routine, and no scent, though `DijkstraMap` is the right tool for it; noise is built, in `docs/design/noise.md`.
  A mind now shoots what it wields when there is a clear shot to take, with `ShootAtRange`, and holds a distance with `Shadow` above it; no game fields a skirmisher yet.
  `UseAbility` scores a footprint at two for a hit and three against for harm, hardcoded in `crates/rl-rules/src/ai/tactics.rs`; make the weights fields.

## 3. Make what exists real

- **A `Burning` entity comes back unlit.**
  `EngineSave` records every burning cell, but the `Burning` component on the entity standing in one is not saved and has no design note saying it should not be, unlike `Fuel`, `LightSource`, `Aware` and `Heard`, which all do.
  A crate that caught fire mid-run reloads without its remaining turns, so `keep_alight` stops refreshing its cell and it burns for whatever the saved field has left rather than for what it had left.
  Mostly self-healing, since the saved field re-catches it, which is why this is at the bottom of the order rather than the top.
- **Every game's log lines said inside a turn go through `Tell`.**
  Corsair, the tutorial, Delve and Heist still push lines straight to `MessageLog` from systems in `TurnSet::React` (the tutorial's "You eat the crust. It helps.", Corsair's portal and discovery lines, the heist's), against the narrator's module doc, so a line can land above the event it answers.
  Each should write a `Tell` instead, and the guide chapters that quote the tutorial move with it; Foundry did this on 2026-09-18.
- **Movement profiles that change costs.**
  `FlowFields::ensure` keys the cache by `MovementProfile` but builds every map with `PathRules::default()`, so a swimmer and a walker see the same map and the sailing profile the plan's river section promised is not wired.
  `TileProps` needs a per-profile walkability mask, and the flood needs to read it.
- **Anyone travels.**
  `WarpRequest` and `GoThrough` ignore everyone but the player (`crates/rl-bevy/src/places.rs`, `resolve_warps`).
  Companions, escorts and a monster fleeing down the stairs are out of reach until a non-player can change maps.
- **A place for a miss.**
  Accuracy is deliberately absent (`docs/design/abilities.md`, "Accuracy does not exist"); the combat docs should say how a game adds a miss as a `DamageStage`, with an example.
- **Light is recast once a frame, not once a turn.**
  `update_lighting` runs in `EngineSet::Light`, after every `Turn` pass the frame ran, so a droid acting in the same frame the player switches a lamp off still sees by the old light, for one turn.
  Foundry's lamp shows it; recasting the dynamic layer inside the turn loop, when a source was added or removed, would close it.
- **`Rooms` can run out of attempts on a small map.**
  Asked for three rooms sized 8 to 10 on a 40x30 map, it fails roughly one seed in sixty inside its default thirty attempts.
  Whether that is a tuning problem, a default `attempts` too low for the room sizes it is asked to fit, or a limit the pass should just document is not yet decided.
  Either way, a test that stamps rooms on a small map has to know this failure rate exists rather than treat every seed as good.
- **Corsair's tests play a different game from its binary.**
  Corsair's binary runs `honour_portals`, `populate_places`, `drop_loot` and `inflict_on_hit`, and its test harness adds none of the four, so no Corsair test exercises them.
  One plugin that both the binary and the harness add, as Foundry's `FoundryPlugin` is, would close the gap for good.
- **A mind cannot use a thing from its bag.**
  A monster could once drink a potion only by using the ability the potion lent, and items stopped lending abilities on 2026-09-23 (`docs/design/effects.md`).
  Throwing a grenade is `ThrowAtRange` and firing a wand is `ShootAtRange` through `Loadout`, as before, but nothing writes `UseItem` for a mind.
  A tactic that uses a thing with a `use` trigger when hurt, read from `Snapshot` the way `ThrowAtRange` reads `missiles`, is the missing piece; no game does it today.
- **A shot has no shape.**
  `RangedAttack` strikes one target, so a scattergun's cone and a lance's beam cannot be written, and a `hit` trigger lands where the one target stands.
  A shape on the shot, from the targeting module abilities already use, is a combat change; `hit` triggers need nothing new once it exists.
- **`on_equip`.**
  Effects land once and wearing is a standing state, which is why `docs/design/items.md` section 6 defers it; a cursed plate that bites when put on is a real case and would be one more moment, reported by the items resolver on `Equip`.
  It waits for a game that wants it.
- **Corsair, Delve and Heist have no map fingerprint tests.**
  A change to their maps goes unnoticed: the Foundry branch changed Corsair's cave maps and Delve's heart floor, and nothing in either game noticed.
  A fingerprint tripwire per game over a few seeds' maps, labelled as such, would make the next such change a deliberate re-baseline.

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

- **The design notes no longer describe the code.**
  Six were mined for the system reference on 2026-09-21 and 2026-09-22 and every one was wrong in a structural claim rather than a detail: `docs/design/minds.md`, `remains.md`, `fields.md`, `noise.md`, `stealth.md` and `props.md` each name a type, a field or a mechanism that has moved or never existed, and two contradict themselves between a section and their own account of what the build changed.
  The pages in `docs/guide/src/systems/` are the accurate description now, and each one's manifest is checked against the files it documents by `scripts/check-systems.py`, which is the thing a note has no equivalent of.
  What a note is still right about is why a subsystem is shaped as it is; the fix is a line at the top of each saying the reference supersedes its model, and the decision about deleting them is `docs/PLAN.md`'s.
- **The worked example teaches an XOR where the rule says `derive`.**
  `docs/guide/src/systems/mapgen.md` spends four sentences of `The line` on a pass's stream coming from `RunSeed::derive`, which `AGENTS.md` names as the mechanism, and the snippet directly above it derives a floor's seed with `RunSeed(self.seed.0 ^ (depth as u64) << 32)` at `examples/tutorial/src/bin/step06_descent.rs:135`.
  `examples/corsair/src/places.rs` does the same with an XOR of its own.
  Both are still derived from the run's seed, so neither breaks the rule about constants or entropy, and the choice of index is the caller's; what is wrong is that the example a reader copies is not the mechanism the page and the guide both name.
  Changing either line changes every map those seeds generate and may disturb fingerprint tests, so it is its own slice rather than a correction to the page.
  Recorded in the same breath: A*'s insertion-order tie-break was documented at `crates/rl-grid/src/astar.rs:10` with no test behind it until 2026-09-22, when writing `systems/grids.md` turned up the claim that ties are pinned by tests and only the flood half was.
  Other documented properties may be unpinned the same way, and a page that claims one is the occasion to check.
- **Two snippets in the reference name no game.**
  Every page in `docs/guide/src/systems/` that quotes an example attributes it, "Warren's floor", "Foundry's probe", "Corsair's `Plunder`", "The tutorial's lantern".
  `docs/guide/src/systems/fields.md` is the exception: both of its `Using it` snippets come from `examples/delve/src/main.rs` and its two lead-in sentences name no game, so a reader of the published book meets two unattributed blocks, the include marker that names the path being hidden in the rendered page.
  The cause was `scripts/check-systems-style.sh`, which banned the word the game is named for until 2026-09-22; `statuses.md` had gone the other way and written "the caves below", which is now "Delve's caves".
  Neither of the two sentences takes a name without being rewritten, since both are general statements of what the snippet is an instance of rather than sentences about a game, so it is a small rewrite of another page's prose rather than a correction, and it waits for whoever is next in that file.
  Found on 2026-09-22 while lifting the ban.

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
- **Struck: view collectors run for screens nobody opened.**
  Measured on 2026-09-22 by `crates/rl-ui/benches/frame.rs`, at thirty-two actors in sight: the map alone is 265 microseconds a frame, the rail a game always shows takes it to 300, and adding the four screen-backed views whose screens nobody has opened takes it to 309.
  Nine microseconds of a 309-microsecond frame, three per cent, against a change that breaks two of the five collectors it would gate: `open_on_crowded_bump` reads `OffersView` every frame to decide whether to open the offers screen, and the menu reads `SheetView` at the end of a run to write the obituary.
  Not worth it.
  `docs/design/ui.md`'s "every frame, not on a turn boundary" stands, and this is a second reason for it.
- **`FlowFields` thrashes rather than evicts.**
  `FlowFields::ensure` (`crates/rl-bevy/src/minds.rs`) clears the entire cache when it reaches thirty-two entries instead of evicting one, and keys it by a `Vec<Point>` of every enemy the asking mind can see, so two hunters seeing different subsets share no flood.
  The doc's promise that fifty hunters after one player cost one flood holds only when all fifty see exactly the same set.
  An LRU, and a coarser key than the full roster, would make it hold more often.
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

## Tracked elsewhere

- The plan's "Next" line: nights on Corsair's surface, scripted encounters, Bevy UI presenters over the panel views, and the living-world-rogue conversion.
- `docs/OVERVIEW.md`, "Not built yet": cursed items, heat and cold, liquids and wind, lit detection ranges, mouse-to-tile, instanced terminal rendering.
