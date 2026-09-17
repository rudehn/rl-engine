# Codebase Concerns

**Analysis Date:** 2026-09-17

This audit cross-checks `docs/TODO.md` (written 2026-09-15 from a review at `cf5f0a9`) and `docs/OVERVIEW.md` "Not built yet" (last updated 2026-09-15 at commit `3ff1c58`).
Items already named in one of those files are marked **Tracked** with the source.
Items found here but absent from both are marked **Untracked**.
HEAD at the time of this audit is `3c44b47`.

The codebase enforces its own rules unusually well: no `TODO`/`FIXME`/`HACK` comments exist anywhere in `crates/`, `examples/` or `templates/`, no crate-wide clippy allow exists, no `#[non_exhaustive]` or `Custom { id }` pattern exists, and `HashMap`/`HashSet` appear in gameplay-adjacent code in exactly two justified spots.
Most of what follows is either debt the project already knows about and has queued, or debt that compounded after the last review because a new example game repeated a pattern the review had already flagged in an older one.

## Tech Debt

**`rl-bevy/src/ability.rs` carries five responsibilities in one file.**
- Issue: state components (`Known`, `Pools`, `Cooldowns`), the effect registry (`EffectKinds`, `add_effect`), the gate (`gate`, `charges_of`, `count_tagged`), payment (`pay`, `spend_charges`, `spend_tagged`), `Offered`, the `Known` refresh, airborne landings and cue emission (`land`, `anchor_of`, `flight_of`, `burst_of`) all live in one 1,717-line file.
- Files: `crates/rl-bevy/src/ability.rs` (functions at lines 68-1046 span all five concerns; `gate` at line 805, `pay` at line 892, `land` at line 769).
- Impact: a change to any one concern risks touching code that reviews the other four; the file is the largest in the workspace by 472 lines over the next largest.
- Fix approach: split into state, registry and resolver submodules with named `SystemParam`s in place of the four-tuple positional destructuring `gate` and `pay` use today.
- **Tracked**: `docs/TODO.md` section 4, "Split `crates/rl-bevy/src/ability.rs`," which measured it at 1,562 lines on 2026-09-15.
  It has grown 155 lines (about 10%) since that measurement and has not been split.

**`TargetView` stores three fields for one sum type.**
- Issue: `ability`, `throwing` and `firing` are three optional fields standing in for one enum, and `Pointing::of` reconstructs the enum from them every frame rather than storing it.
- Files: `crates/rl-ui/src/view/target.rs` (1,085 lines; `collect_target` at line 468 runs 100 lines, `aim_cursor` at line 299 runs 92 lines).
- Impact: every reader of `TargetView` has to know which of the three fields is live instead of matching one enum, and a game cannot extend what the cursor aims without editing the struct.
- Fix approach: store `Option<Pointing>` and open `Pointing` so a game's own aim (a direction to dig, someone to talk to) rides the same cursor.
- **Tracked**: `docs/TODO.md` section 4, "`TargetView` holds the enum it keeps reconstructing."

**`OnMap` is optional everywhere a map boundary is checked.**
- Issue: `on.map(|m| m.0).unwrap_or(MapId::SURFACE)` (or the `Option<&OnMap>` equivalent) is repeated at 24 call sites across the engine and both non-tutorial example games, each one re-deciding what "no `OnMap`" means.
- Files: `crates/rl-bevy/src/turn.rs:300,348`, `crates/rl-bevy/src/doors.rs:126`, `crates/rl-bevy/src/minds.rs:317`, `crates/rl-bevy/src/items.rs:295,314,529`, `crates/rl-bevy/src/status.rs:120`, `crates/rl-bevy/src/stealth.rs:177,294`, `crates/rl-bevy/src/places.rs:232,233,284`, `crates/rl-bevy/src/lighting.rs:202`, `crates/rl-render/src/map_view.rs:322`, `crates/rl-ui/src/focus.rs:94`, `crates/rl-ui/src/view/inspect.rs:209`, `crates/rl-save/src/run.rs:228`, `crates/rl-save/src/engine.rs:148`, `examples/corsair/src/input.rs:142`, `examples/delve/src/main.rs:623`, `examples/heist/src/main.rs:622,828`, `examples/tutorial/src/bin/step06_descent.rs:536`.
- Impact: a new call site is one more place that has to remember the `SURFACE` default; the tutorial's first chapter has to explain to a beginner why a delve's first floor is "map one" purely to justify this default.
- Fix approach: require `OnMap` on `Position` (or give it a `Default`) so the `Option` and its fallback disappear at every site.
- **Tracked**: `docs/TODO.md` section 4, "`OnMap` as a required component."
  The count of 24 sites here is higher than the review's informal list ("turn, items, minds, places, status and both games"), confirming the pattern is still spreading rather than shrinking.

**A game's own draws bypass `Seed::stream` in two places, not one.**
- Issue: the randomness rule requires a game's own rolls to come from `Seed::stream`, never from the engine's internal per-subsystem stream resources (`CombatRng`, `AbilityRng`) that a game should not see in its own prelude.
  Corsair's loot drop and its status-infliction roll both draw from `ResMut<CombatRng>` directly.
- Files: `examples/corsair/src/items.rs:327` (`drop_loot`), `examples/corsair/src/statuses.rs:25` (`inflict_on_hit`).
- Impact: Corsair's own draws are not derived through its own named domain, so replaying a recording after any engine change to how much `CombatRng` draws per turn can desync Corsair's loot and status rolls even though the fingerprint tripwire (`rl-bevy/tests/fingerprint.rs`) does not cover a downstream game.
- Fix approach: remove `CombatRng` and `AbilityRng` from the preludes as the TODO proposes, and give both call sites their own `Seed::stream` domain.
- **Tracked**: `docs/TODO.md` section 4, "Streams out of the prelude," which names only `items.rs`'s `drop_loot`.
  The `statuses.rs:25` occurrence is the same violation in a second file and is **untracked**.

**Movement profiles do not change pathing costs.**
- Issue: `FlowFields::ensure` keys its cache by `MovementProfile` (the `FieldKey` tuple includes it) but always builds the field with `PathRules::default()`, so a swimmer and a walker read the same cost map regardless of the profile in the key.
- Files: `crates/rl-bevy/src/minds.rs:99` (`FieldKey` includes `MovementProfile`), `:127-146` (`ensure`, which never reads the profile before calling `field.build(&view, locals, PathRules::default())` at line 143).
- Impact: the cache key promises per-profile behaviour it does not deliver; a river or lava tile meant to block a walker but not a swimmer will not, silently, because nothing reads the profile.
- Fix approach: give `TileProps` a per-profile walkability mask and have the flood read it when building `PathRules`.
- **Tracked**: `docs/TODO.md` section 3, "Movement profiles that change costs."

**Only the player can change maps.**
- Issue: `WarpRequest` and `GoThrough` resolution only considers the player entity, so a companion, an escort, or a monster fleeing down the stairs cannot follow.
- Files: `crates/rl-bevy/src/places.rs` (`resolve_warps`).
- Impact: any game wanting a companion or an escort mechanic (Corsair's crew, a delve's monster fleeing) cannot use the built-in transition system for anyone but the player.
- **Tracked**: `docs/TODO.md` section 3, "Anyone travels."

**Attack cost is a constant, not a weapon property.**
- Issue: `resolve_attacks` charges the flat `BASE_ACTION_COST` for every blow regardless of weapon; accuracy does not exist at all (a deliberate design choice per `docs/design/abilities.md`), but the cost side of that choice has no worked example yet.
- Files: `crates/rl-bevy/src/combat.rs:389,407` (`resolution.done(intent.actor, rl_core::turn::BASE_ACTION_COST)`).
- Impact: the most common combat knob (a fast dagger vs. a slow greatsword) is unavailable without editing the engine's combat resolver.
- **Tracked**: `docs/TODO.md` section 3, "Attack cost, and a place for a miss."

**`DamageStages` defaults to empty rather than to a safe default.**
- Issue: a game that forgets to register `DamageStages` gets raw, unmitigated damage with no warning, which is exactly the "a resource happens to exist" failure mode `app.needs` was built to prevent elsewhere.
- Impact: silent, hard-to-diagnose balance bugs in a new game (every hit does full damage, armor does nothing) with no message pointing at the cause.
- Fix approach: default to `SubtractArmor`, or gate it with `needs` like every other subsystem's game-supplied requirement.
- **Tracked**: `docs/TODO.md` section 3, "`DamageStages` must not default to empty."

**AI tactic weights are hardcoded rather than data.**
- Issue: `UseAbility::score` weighs a hit worth using at `+2` and harm to an ally at `-3` as literal constants, and no pack, leader, keep-at-range, patrol, idle, noise or scent tactic exists yet even though `DijkstraMap` is already the right tool for the last two.
- Files: `crates/rl-rules/src/ai/tactics.rs:301-324` (`score`, with the `worth * 2 - harm * 3` line at 323).
- Impact: every game that wants a different risk tolerance for its minds has to fork the tactic rather than configure it.
- Fix approach: make the weights fields on the tactic, loaded like every other numeric knob in `rl-rules`.
- **Tracked**: `docs/TODO.md` section 2, "Tactics that are missing, and weights that are fixed."

**Doc comments carry development history instead of only "why" and "why-not."**
- Issue: `docs/PLAN.md`'s progress log is the place for how a decision came to be; several doc comments across the Bevy layer narrate that history inline instead, at a greater density than `rl-core/src/turn.rs`, the project's own stated bar.
- **Tracked**: `docs/TODO.md` section 5, "Doc comments at `turn.rs` density."

**A returning reader has no plugin reference table.**
- Issue: `docs/OVERVIEW.md`'s `rl-bevy` section is prose paragraphs of a hundred to two hundred words per plugin, with no table of plugin, what it needs, what it adds and what it emits.
- Files: `docs/OVERVIEW.md` lines 87-124 (the `rl-bevy` section).
- **Tracked**: `docs/TODO.md` section 5, "A plugin table in the overview."

## Fragile Areas

**Heist repeats the exact unmodularized-`main.rs` shape the review already flagged in Delve.**
- Files: `examples/heist/src/main.rs` (1,076 lines, only 2 source files and 2 files carrying `#[test]` in the whole crate), `examples/delve/src/main.rs` (1,131 lines, 3 source files, 2 with tests).
- Why fragile: both games hold input handling, floor population, narration wiring and content loading in one file each, unlike Corsair's 12-file, 8-tested-file layout (`examples/corsair/src/*.rs`).
  `populate_floor` alone runs 98 lines in Heist and 97 in Delve (heuristic scan), the kind of function that is hard to unit test in place and easy to break when a game author copies it as a starting point.
- Safe modification: treat the file as several concerns bundled together (spawning, narration, input) before adding to it; anything genuinely new belongs in its own module the way Corsair's `content.rs`, `items.rs`, `statuses.rs`, `quests.rs` and `input.rs` are split out.
- Test coverage: Heist's own gameplay logic (stealth-and-light interplay, the sound-propagation mechanic) has no dedicated test file; its 2 tested files are shared scaffolding, not the mechanic the example exists to demonstrate.
- **Untracked**: `docs/TODO.md` section 5 names only Delve's `main.rs` ("Split the delve's `main.rs` into input, narration and content modules the way Corsair is") because Heist did not exist yet when the review was written (`8b6487f`, the Heist commit, lands after `cf5f0a9`, the review's base commit).
  The fix for Delve, if applied without also covering Heist, will leave the newer example carrying the same debt.

**`crates/rl-bevy/src/minds.rs` and `crates/rl-rules/src/ai/tactics.rs` are both large, single-purpose-but-many-branch files.**
- Files: `crates/rl-bevy/src/minds.rs` (1,245 lines), `crates/rl-rules/src/ai/tactics.rs` (954 lines).
- Why fragile: the AI decision pipeline (perceive, roster, filter, annotate, decide) and the tactic-scoring logic it feeds both live in dense files with many small functions that share mutable state (`Thinking`, `Snapshot`) threaded through several call sites; a change to one tactic's scoring shape risks an unintended change to another tactic that reads the same snapshot fields.
- Safe modification: read the `PerceiveSet` ordering comment at the top of `crates/rl-bevy/src/minds.rs` before adding a new contributor, and add new tactics as new functions rather than branching inside `score`.
- Test coverage: `crates/rl-rules/src/ai/tactics.rs` has extensive seeded unit tests (lines 598-947 are a block of `StdRng::seed_from_u64` tests), which mitigates but does not eliminate the risk of the shared-state coupling above.

**`collect_narration` is a 176-line single function, the longest in the workspace by line count.**
- Files: `crates/rl-ui/src/narrate.rs:282` (`collect_narration`, `narrate.rs` overall is 820 lines).
- Why fragile: one function resolving every `Said` row into a phrasebook lookup, perspective split and colour-naming pass in one pass makes it hard to test a single phrase's wording without exercising the whole collector.
- Safe modification: the phrasebook itself (`Phrasebook`) is data-driven and safe to extend; changes to the collection loop's control flow are the risky edits.

**`resolve_items` (120 lines) and `step_fire` (104 lines) are the largest single system functions outside `ability.rs`.**
- Files: `crates/rl-bevy/src/items.rs:292` (`resolve_items`), `crates/rl-bevy/src/fire.rs:252` (`step_fire`).
- Why fragile: both resolve several action variants or several fire-adjacent effects (catching, spreading, smoking, extinguishing) in one match-heavy function; a new action or fire behaviour is most naturally added as another branch, growing the function further rather than shrinking it.

**`FlowFields` correctness silently depends on nobody using more than one movement profile.**
- See "Movement profiles do not change pathing costs" above.
- Why fragile specifically: since the cache key already differentiates by profile but the built field does not, a future contributor skimming `FieldKey`'s definition (line 99) would reasonably assume the profile is honored; the bug is invisible until a game actually defines two profiles with different walkability and observes both share a map.

## Duplication Between Examples and Engine Crates

**Three example games each hand-roll the "carried item is off-map" and "map-scoped lookup" pattern instead of the engine giving them one helper.**
- See "`OnMap` is optional everywhere" above; this is both a tech-debt item (the `Option` itself) and a duplication item (24 independent inline implementations of the same map-scoping check, spread across every engine crate that touches position and all three non-tutorial example games).

**Corsair's ability effect (`Plunder`) and Delve's (`Drain`) follow identical registration shape but live in unrelated files with no shared test.**
- Files: `examples/corsair/src/abilities.rs:57-85` (`Plunder`, registered at `examples/corsair/src/main.rs:140`), `examples/delve/src/*.rs` (`Drain` in `effects.rs`).
- Impact: low by itself (this is the intended extension seam, `add_effect`), but the only cross-game test that the seam holds is `crates/rl-bevy/tests/genres.rs`, which is engine-side; neither example's own test suite proves its custom effect keeps working as the engine's effect registry evolves.

**RNG-stream misuse duplicated across two files in one game.**
- See "A game's own draws bypass `Seed::stream`" above; the same anti-pattern (`ResMut<CombatRng>` instead of `Seed::stream`) is now in two files (`items.rs`, `statuses.rs`), meaning a single fix pass has to catch both or the second one will look like precedent for a third.

## Performance Bottlenecks

**Terminal rendering spawns one `Sprite` and one `Text2d` entity per cell.**
- Files: `crates/rl-render/src/terminal.rs:190-213` (grid setup spawns a `Sprite::from_color` background entity and a `Text2d` glyph entity for every cell in the terminal).
- Problem: an 80x40 terminal is 3,200 cells, meaning 6,400 persistent entities each individually updated and drawn every frame Bevy's renderer processes them; this does not re-spawn per frame (the entities persist and are mutated via `Query<&mut Sprite>` and `Query<(&mut Text2d, &mut TextColor)>` at lines 224-225), so the cost is steady-state draw-call and query overhead rather than allocation churn, but it does not batch into fewer draw calls the way an instanced glyph buffer would.
- Cause: no instanced rendering path exists; each cell is a full ECS entity with its own transform and renderable.
- Improvement path: an instanced terminal renderer (a single mesh with per-instance glyph/color data) would collapse thousands of entities and draw calls into one.
- **Tracked**: `docs/OVERVIEW.md` "Not built yet," "Instanced terminal rendering; one sprite per cell is the known scaling limit."

**No headless benchmark exists for the Bevy-layer turn loop, minds decision pass, or terminal draw pass.**
- Files: the only `benches/` directory in the workspace is `crates/rl-grid/benches/grid.rs`, covering shadowcasting, `DijkstraMap`, `SpatialGrid` and lighting.
- Impact: `rl-bevy`'s `Turn` schedule (which can run many passes per frame), the minds `PerceiveSet` pipeline, and `rl-render`'s per-frame diff-and-draw have no criterion coverage, so a regression in any of them (e.g. a `Thinking` snapshot rebuild that becomes O(n^2) as actor count grows) would only surface as a felt frame-rate drop in a live game, not a failing benchmark in CI.
- Priority: medium - the tier-1 crates (`rl-grid`, `rl-mapgen`, `rl-world`) that do the heaviest one-time generation work are well benched; the steady-state per-frame cost of the tier-2 loop is not.

## Test Coverage Gaps

**Heist's stealth-and-light interplay, the mechanic the example exists to demonstrate, has no dedicated test.**
- What's not tested: sound propagation to `Aware` on a thrown pebble, a watchman's notice being shared with everyone in earshot, and the lantern-shading choice as a player-facing mechanic.
- Files: `examples/heist/src/main.rs` (1,076 lines, 2 tested files in the crate, both shared scaffolding rather than heist-specific mechanics).
- Risk: a refactor of `StealthPlugin` or `LightingPlugin` could silently break the one worked example that exercises them together, since `crates/rl-bevy/tests/fingerprint.rs` (the engine's own tripwire) does not include Heist's content.
- Priority: medium.

**Delve's dungeon-specific mechanics (`Drain`, fuel-vs-shift lighting interaction, gut-eel spit-back) are exercised by only 2 of 3 files carrying tests.**
- Files: `examples/delve/src/main.rs` (1,131 lines).
- Risk: same shape as the Heist gap above - the file most likely to be edited when adding delve content is the file least covered by tests.
- Priority: medium.

**No test proves `FlowFields` honors (or fails to honor) a second `MovementProfile`.**
- Files: `crates/rl-bevy/src/minds.rs` (`FlowFields`, `FieldKey`).
- Risk: because the bug described above ("Movement profiles do not change pathing costs") produces no compiler warning and no failing existing test, it will stay latent until a game defines two profiles and a QA pass or player notices a swimmer refusing to cross water it should cross.
- Priority: high, specifically because fixing "Make what exists real" (`docs/TODO.md` section 3) without first writing the property test risks re-introducing the same silent gap.

## Doc Drift Against Code

**`docs/OVERVIEW.md` is current as of `3ff1c58`, three commits behind `HEAD` (`3c44b47`).**
- The commits between them touch `crates/rl-bevy/src/ability.rs` (registering three shared messages `AbilitiesPlugin` now writes through), `crates/rl-engine/src/lib.rs` (a wasm canvas-fit window flag), `crates/rl-render/src/map_view.rs` (a new `MapView::clamp_to` and an updated `follow_player` that keeps the camera inside map bounds), and `crates/rl-save/src/morgue.rs` (a wasm-only `FileBackend` import gate).
- Assessment: none of these rise to "adds or removes a system" in a way the overview's inventory currently misses at the level of detail it already keeps (the overview does not enumerate `MapView` methods individually), so this is low-severity drift, not a broken invariant.
  It is worth a one-line mention next time `docs/OVERVIEW.md` is touched, since `follow_player`'s new map-clamping behaviour is a small but real behavior change to what every game already sees on screen.

**Illustrative example words in engine-crate doc comments brush against the "no theme words in docs" rule.**
- Files: `crates/rl-rules/src/affix.rs:1,50,52,167,168,299,358,415,416` (uses "cutlass" repeatedly as the worked example for affix naming), `crates/rl-bevy/src/combat.rs:18,261`, `crates/rl-bevy/src/items.rs:10,218`, `crates/rl-bevy/src/ability.rs:189,191,192,202,203,285,1098,1540,1677` (uses "sword," "potion," "wand" as illustrative examples), `crates/rl-save/src/morgue.rs:39` ("`Killed by the goblin`" as a sample obituary line), `crates/rl-ui/src/narrate.rs:644` ("`the goblin`" as a sample narrated line).
- Assessment: `CLAUDE.md`'s rule reads "No fantasy, sci-fi or pirate vocabulary in types, docs or constants," and these are all doc-comment prose, not types or constants, so this sits in genuine tension with a literal reading of the rule.
  In every instance found, the word illustrates what a *game* might name something (a damage kind, an item, a log line) rather than naming an engine concept itself, and the file `crates/rl-ui/src/lib.rs:73` states the rule in the same style ("No engine type, doc or constant says weapon, spell...").
  Severity: low.
  This is a style question for whoever next edits these files rather than a structural defect, since removing the illustrations would make the affix and ability docs harder to follow without a worked example.

## Security Considerations

**None found specific to this codebase's threat model.**
- This is a game engine with no network layer, no server-side trust boundary, and no user-supplied code execution path beyond RON content files loaded from the game's own asset directory.
- The one boundary worth naming: `rl-save`'s file backend (`crates/rl-save/src/backend.rs`) writes and reads save files by name under a directory beside the executable; nothing in the reviewed code sanitizes a save-slot name before it reaches the filesystem path, though every current caller passes a compile-time or engine-generated slot name rather than unsanitized user text.
  Not exploitable today; worth a note if a future game ever lets a player type a save name freely.

## Scaling Limits

**Terminal size is the practical ceiling until instanced rendering exists.**
- Current capacity: comfortable at the sizes every current game and the tutorial use (roughly 80x40 to 100x50 glyph cells, per `RoguelikePlugins::new(title, cols, rows)` call sites).
- Limit: the one-sprite-one-text-per-cell approach (see Performance Bottlenecks above) means draw cost scales linearly with cell count with a meaningfully large constant factor (two entities per cell); a much larger terminal (a wide, high-resolution ASCII display) would be the first thing to feel it.
- Scaling path: instanced terminal rendering, already named in `docs/OVERVIEW.md` "Not built yet."

## Dependencies at Risk

**The `rl-` crate name prefix cannot be published under its current names.**
- Risk: `rl-core` collides with an unrelated, already-published crate on crates.io (a token-bucket rate limiter at version 1.22.0), so the entire family's naming scheme has to change before a first public release.
- Impact: 297 references across 153 files and the public path `rl_engine::rl_core::Rect` that the guide teaches would all need updating in one renaming pass.
- Migration plan: `docs/TODO.md` section 6 lists candidate whole-family names already checked for availability (`roguelike`, `dungeoneer`, `torchlit`, `runedeep`, `vaults`, `morgue`) and recommends reserving the chosen family immediately once picked.
- **Tracked**: `docs/TODO.md` section 6, "The name is taken."

**51 internal path dependencies carry no version requirement, blocking `cargo publish`.**
- Risk: none for development, but the workspace cannot be published to crates.io in its current form; `rl-engine`'s `readme = "../../README.md"` also points outside its own package, which `cargo package` refuses outright.
- **Tracked**: `docs/TODO.md` section 6, "What blocks `cargo publish` is version requirements, not metadata."

**API churn makes any publish premature.**
- Risk: 77 commits touched crate sources in the 30 days before 2026-09-17, changing roughly 2,100 lines of public declarations; `CHANGELOG.md` records releases, but not every public change between them.
- **Tracked**: `docs/TODO.md` section 6, "Nothing should go out while the API moves this fast."

## Known Bugs

**None found that are not already covered above.**
- The two candidate "bugs" this audit turned up - the `FlowFields` movement-profile no-op and the `CombatRng` stream leak - are filed under Tech Debt and Fragile Areas above because they are architectural gaps (documented, non-crashing, silently-wrong-by-design-omission) rather than crashes, panics or data-loss bugs.
- No panics, unwraps-on-untrusted-input, or off-by-one errors were found during this pass; the codebase's `unwrap()` calls that were sampled are all on values the surrounding code has just established are `Some`/`Ok` (e.g. registry lookups after a validate-on-load pass), consistent with the project's "validate once, trust after" content-loading design.

---

*Concerns audit: 2026-09-17*
