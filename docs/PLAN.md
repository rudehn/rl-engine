# rl-engine extraction plan

Status: adopted, revised 2026-09-09 after Nate's review; being built.

## Progress

- 2026-09-10: M0, M1 and M2 are built and committed; every crate through `rl-ai` exists, tier 1 is Bevy-free by CI.
- 2026-09-10: WildReach is abandoned as a repo.
  Corsair, the pirate example inside this workspace, is the consumer the milestones are built against from here on.
  The `wildreach` directory is left as it was and is not maintained.
- 2026-09-10: the turn loop runs every actor due before the player's next turn inside one frame (the `Turn` schedule and `EngineSet::Input`), after a Corsair run showed one actor per frame.
- 2026-09-10: M3 first slice: the slot graph in `rl-rules`, items in `rl-bevy` (ground, bag, slots, stacks, use as a game event), the list menu in `rl-ui`, and Corsair's armory, loot drops, rum and sea chest.
  Deferred from M3: affixes and enchant, throwing, the character sheet.
- 2026-09-10: M4 first slice: `rl-mapgen` gains rooms, BSP, doors, random start, farthest exit and prefab stamping; `rl-bevy` gains places (`MapId`, `OnMap`, `Transition`, `WarpRequest`, `PlaceRules`, map-scoped occupancy and knowledge, frozen off-map actors); Corsair gains huts in its ports, cave mouths on its coves, two-level smugglers' caves with a treasure vault, and portals that warp out of a cave.
  Deferred from M4: rivers and bridges inside chunks beyond the channel pass, cullers beyond keep-largest, a choke map, decoration rules, settlement passes richer than huts.
- 2026-09-10: M5 first slice: `rl-events` (facts with kind, subject, object and amount; matchers; a ledger of named counters; quests as objectives over facts with `after` chains and a `victory` flag, tracked purely), `rl-bevy` wraps it as the `Happened` message, the `Quests` and `Counters` resources and `QuestChange`, fed after the frame; the dead now linger until the end of the frame so reactions can read what they were.
  Corsair reports kills, pickups, what it carries, cave levels and sites as facts, loads four tasks from RON that chain to "Retire rich", narrates them, draws a ledger on `t`, and wins the run when the hoard comes home.
  Deferred from M5: scripted encounters, abilities and targeting, `TileField<T>`, lighting, a generated victory condition.
- 2026-09-10: M6 first slice: `rl-save` (the `SaveBackend` seam with file, memory and `localStorage` backends, the `Versioned` envelope with an exact-match policy, `EntityRemap` and `SaveId`, and `EngineSave` capturing and restoring the scheduler, the world's edits and places, and knowledge), `rl-tools` (`ThreatSubject`, threat, and the spawn-band `Report`).
  Corsair saves with `S`, saves and quits with `q`, continues with `--continue`, deletes the save on death, and prints its balance report with `--balance`.
  Deferred from M6: the wasm `beforeunload` bridge, seed replay, the headless dump.
- 2026-09-10: deferred pieces, first pass: `rl-rules` gains the affix and enchant model (tags, `AffixDef` with level-scaled stat grants and extra strikes, `EnhanceRule`, `Enchanted` per instance, weighted rolling); `rl-grid` gains targeting (`TargetMode` own, adjacent, bolt, ball, beam, cone; `footprint`; `clear_shot`); `rl-bevy` gains `RangedAttack`, `Strikes` and `line_of_fire`, and the attack resolver shoots at distant targets.
  Corsair's items carry tags and affixes from RON, found gear rolls a quality, the hoard is enchanted, the pistol fires on `f`, and the sea chest shows the folded numbers.
- 2026-09-11: statuses in the Bevy layer: `StatBlock` and `Afflicted` components, `Afflict` and `Cure` requests, `StatusEvent`, and a tick on every whole turn for the actors on the current map whose damage goes through the damage pipeline; `is_status_source` and `Stats::retain_sources` let a game rebuild gear modifiers without losing a status's.
  Corsair's crabs and jaguars inflict bleeding and venom from RON, rum cures both and makes you hearty, and the status line wears badges.
- 2026-09-11: chunks are painted per tile, not per region.
  `rl-world` gains `fine.rs`: bilinear interpolation of climate and water distance between region centres, read at a position moved by a low-frequency warp so biome edges wander, plus a tile-scale clump field; `WorldGraph::tile_facts` and `clump_at`, and `ChunkContext` carries `facts` and `clumps` next to `heights`.
  `rl-mapgen` gains `ScatterBy`, a scatter whose chance is a function of the context and the cell.
  Corsair classifies each tile with the same rule the region got and grows trees from moisture shaped by the clumps; the region band remains the overworld's, the sites' and the spawn tables'.
  Nate, 2026-09-11: "a forest just looks like a 64x64 block" was the prompt.
- 2026-09-11: the surface is optional and a delve is first-class.
  `stream_chunks`, the viewshed's site discovery and the warp take the world graph as an option; `PlaceRules::build` gets `Option<&WorldGraph>`; `PlaceBuild::from_context` turns a finished chain into a build; `WarpRequest::into_place` starts a run in a place.
  The second example, `examples/delve`, is the Hollow Whale: five floors, no surface, one file of chains.
  Nate, 2026-09-11: dungeon map building should be first-class and easy.
- 2026-09-11: lighting, phases A and B of `docs/design/lighting.md`.
  `rl-grid` gains `light`: intensity and colour as separate channels, integer falloff to zero at the rim, screen blending, order-independent casting through the shadowcast, a flood and a compose; benched at twenty sources.
  `rl-bevy` gains `Lighting` (opt-in), `LightSource` for props, actors and items alike with carried items shed from the carrier, `DarkSight`, `Fuel` and `LightEvent`, the map's `opacity_epoch`, and the gate: a viewshed keeps its `line` and its `visible`, and minds perceive along a line only what is lit, within dark sight or adjacent.
  `rl-render` tints by the landed colour and draws intensity as digits on request.
  The third example, `examples/lamplight`, is one cave with a lantern, a brazier, wisps, a torch and lurkers.
  Nate, 2026-09-11: no day cycle forced on a delve, lights on items, monsters and props, and a basic example.
- 2026-09-11: the map is drawn the way Brogue draws one, and the examples go dark.
  `rl-grid`'s `Light` gains a `waver` channel that only the renderer reads, fed by a source's `flicker`.
  `rl-render` gains `shade`: tiles authored with both colours and a per-cell `Vary` that can shimmer, light that multiplies glyph and background, flicker on a smooth noise, and `Memory` that fades to cold blue; and `capture`, which plays keys through the real input, photographs the window and refuses a black frame.
  The delve turns lighting on (grey daylight in the Maw, darkness below, a brand, glowing bile, beasts' glow and dark sight from RON, `--floor`); Corsair's surface is daylit and its caves dark, with the player's lantern lit on the way down and lanterns on cutthroats and marines from RON; Lamplight gains fungus, pools and flames.
  Found on the way: Lamplight's `h` toggled the heat map before movement read it, so walking west with `h` never worked; the heat map is on `v` now.
  Nate, 2026-09-11: "I want the lighting to look like this screenshot", a Brogue screenshot.
- 2026-09-11: the crate layout follows its own rule, and places leave the overworld.
  `rl-content`, `rl-events`, `rl-ai` and `rl-tools` become the `content`, `events`, `ai` and `balance` modules of `rl-rules`: boundaries are drawn on dependency weight (section 4), and all five weighed the same, core and grid and serde and ron.
  Every item is re-exported at the crate root, so a path loses a crate and gains a module at most.
  `rl-test-support` is deleted rather than kept: nothing had used it in six milestones, `rl-grid` already carries its own ASCII fixtures, and the seed loops already name the seed they failed on.
  `WorldMap::new` takes the tile tables alone; the streamed surface is an inner part that learns its region size from the world graph on the first load, and `Knowledge` needs no construction at all.
  A delve now names no region size anywhere.
  Found on the way: the overworld's fog read the explored tiles of whatever map was current, so a player underground saw a cave's buckets drawn as surface regions; the regions seen are now recorded apart from the per-map tiles, and the places test asserts it.
  Nate, 2026-09-11: the architecture review's crate-layout and region-size findings.
- 2026-09-11: actions stop being an enum.
  `Action` is a trait, `Intent<A>` the message that carries one, and each action is a type owned by the module that resolves it: `Step` and `Wait` in the turn loop, `Attack` in combat, the five item actions in items, `GoThrough` in places.
  A game registers its own with `App::add_action`, resolves it in `TurnSet::Resolve`, and claims the actor through the `Acting` resource, which is what keeps one turn to one action now that the resolvers are separate.
  The new `TurnSet::Sweep` refuses an intent no resolver claimed and says which type it was, so a forgotten resolver reads as a refusal and a warning rather than a player frozen holding a turn nothing will spend.
  Minds claim their decision too, so a game that decides for an actor in `TurnSet::Decide` keeps the mind from overriding it: a monster can now take an action the engine has never heard of, which the closed enum made impossible.
  Two names dodge Bevy's prelude: `Move` and `Enter` are taken by `bevy_picking`, so the engine's are `Step` and `GoThrough`.
  Nate, 2026-09-11: the architecture review's closed-taxonomy finding.
- 2026-09-11: a game's rules move inside the turn.
  `TurnSet::React` runs between the sweep and the requeue, and `PresentSet` names the drawing layers: narrate, map, chrome, overlay.
  Corsair's drink, loot, gear and venom, and the floor and cave population of all three games, ran in `EngineSet::Present`, which is after every pass of the turn loop; a potion drunk at one hit point healed a corpse.
  Reproduced first as a Corsair test with a cutthroat at the player's elbow, which died before the fix and lives after it.
  `rl-ui` and `rl-overworld` ordered themselves after `rl_render::map_view::draw_map` by name; they name a sub-phase now, and the overworld's own systems are public so a game can reason about them at all.
  Nate, 2026-09-11: the architecture review's reaction-phase finding.
- 2026-09-11: `docs/design/ui.md`, the UI plan, written and section 3.12 revised to match it.
  Every panel splits into a view, a collector and a presenter, so what a game reuses is the query into a model and not the drawing; the engine names no content and no colour, `LogCategory` becomes an interned `ToneId` over a palette a game extends, and a row carries facets the game pushes for what the engine cannot know.
  The backend question that section 3.12 answered with Bevy UI and the built code answered with the glyph terminal is settled by not being decided first: the views do not know which backend draws them, terminal presenters ship now, node presenters ship when a game asks.
  Opt-in is per panel, not per crate, and `EnginePlugins` gains nothing.
  Nate, 2026-09-11: the nearby list, equipment and stats, inspect and the log should be common features of the engine, without the theming being common too.
- 2026-09-11: the engine's subsystems become plugins, and opt-in means opt-in.
  `EnginePlugins` registered everything and three subsystems decided for themselves whether to run by looking for a resource, so a game that forgot `CombatRng` got monsters that stood still and no word about why.
  `CorePlugin` now holds the loop, the clock, the map, its places and the three actions that need nothing else; `FovPlugin`, `CombatPlugin`, `StatusPlugin`, `ItemsPlugin`, `LightingPlugin`, `StreamingPlugin` and `FactsPlugin` are added by name.
  Each says what it needs: a missing rule table panics on entering play naming the plugin and the resource, and a plugin whose dependency is absent panics at build naming both.
  `TurnSet::Resolve` gains `ResolveSet` (act, effects, damage) because the systems that fill it now come from different plugins and cannot chain themselves.
  Nate, 2026-09-11: the architecture review's implicit-configuration finding.
- 2026-09-11: the facade's prelude is worth globbing.
  It re-exported core and grid only, and nothing in the workspace used it: the games imported from eight or more paths each.
  It now covers core, grid, mapgen, world, rules, the Bevy layer, render, UI, overworld and save, and `rl-ui` and `rl-overworld` gained the preludes they never had.
  `Rect` is left out on purpose, since Bevy's prelude has its own and a game globbing both would disambiguate every use; a doc test globs both preludes and names a type from each crate, so the next collision fails there.
  The delve and Lamplight glob it, and their engine imports collapse to one line; Corsair still names what each of its ten modules uses, which is its own documentation.
  Nate, 2026-09-11: the architecture review's prelude finding.
- 2026-09-11: the two loose ends of the review.
  A brain could only choose from the engine's three decisions, so an action a game added could only ever be taken by the player; `Decision::Game(u32)` carries a number in the game's own numbering, the way a map's spots are tagged, `MindChose` reports it, and `DecideSet` splits the decide phase so the game answers after the minds have chosen.
  A monster can now shove, and a game's tactic sits anywhere in the priority list beside the engine's.
  `OverworldPlugin` and `ChromePlugin` drew nothing at all when their layout resource was missing; they say so on entering play now, through the same `needs` helper the engine's own plugins use, which is public for that reason.
  Nate, 2026-09-11: the last two findings from the architecture review.
- 2026-09-12: the panels, phases A to F of `docs/design/ui.md`.
  `rl-ui` is rebuilt around the three-way split: `tone` (an interned `ToneId` over a `Palette` a game extends, with every uncoloured tone named at startup), `facet` (a note a game pushes onto a row in `ViewSet::Annotate`), `modal` (a stack of interned ids with `modal_is`, `modal_open` and `no_modal`), `keys` (the eight directions off arrows, vi keys and the numpad), `view/` (`NearbyView`, `VitalsView`, `GearView`, `InspectView` and a collector plugin each) and `panel/` (a terminal presenter each, plus the frame, heading, bar, clip and rectangle-split helpers a game writing its own reaches for).
  `rl-rules` gains `forecast`: expected damage with the average roll put through the game's own mitigation, blows and turns to fell either side, and an `Outlook` read off the two counts, so an inspect panel's numbers cannot drift from the fight.
  `rl-bevy` gains `Slots`; `Theme`, `LogCategory`, `ChromePlugin`, `ChromeLayout` and `StatusLine` are gone, and `rl-overworld` moved its open flag onto the shared stack so it and a game's screens cannot both own the arrow keys.
  Corsair takes the full set with a rail down the right and a facet for what an enemy wields; the delve and Lamplight take vitals and a log and nothing else, which is the point.
  The guide gains `10-panels.md` and `examples/tutorial/src/bin/step10_panels.rs`; testing and where-to-go-next renumber to 11 and 12.
  Found on the way: `VitalsViewPlugin` asserted on `StatusRules`, so a game with no statuses had to insert an empty registry to get a health bar; and the look cursor settled on the player when nothing else was in sight, so the panel forecast a duel with yourself. Both fixed, both tested.
  Phase G in the same slice: the scrollback, a second presenter over the same `MessageLog` the strip draws, with its own cursor and filter, lines wrapped rather than clipped, each turn ruled off with its number, and a filter that cycles only the tones the log holds. Two screens over one view, neither aware of the other, is the split proving itself.
  Deferred: Bevy UI presenters over the same views (phase H), which waits for a game that asks for wrapping, hover or sub-cell bars.
  Nate, 2026-09-11: the nearby list, equipment and stats, inspect and the log should be common features, without the theming being common too.
- 2026-09-12: abilities, phases A to C, E and F of `docs/design/abilities.md`.
  `rl-rules` gains `ability`: `AbilityDef` with an `Aim`, a shape, costs, requirements, an integer time and cooldown and a list of named effects; `Lookup`, so a game authors abilities by name against its own registries instead of mirroring the schema in a type of its own; `blocked`, which reports every reason a use is refused rather than the first; and the `UseAbility` tactic, which scores a footprint by what the `Aim` wants under it so a mind fires an ability it cannot understand.
  `rl-bevy` gains the `Use` action, `Known` rebuilt each turn from what an actor is and wears, `Pools`, `Cooldowns` as absolute times on the turn clock, `Grants` and `Charges`, `Offered` so the gate runs once for the deciding mind, and the resolver.
  Effects are types, not a list, which is `10e3d82`'s answer one level down: `Harm` and `Mend` in combat, `Inflict` and `Cleanse` in status, `Shove`, `Pull` and `Teleport` beside the resolver that owns the movement, and `add_effect` for a game's own. An effect asks through `EffectWorld` and reaches anything else through `Commands`; there is no closed enum of effect kinds and no `Custom { id }`.
  The fourth example, `examples/knacks`, is one arena and five genres of ability in one registry, differing in nothing but RON, with the five effects the engine does not ship in a file beside them.
  Found on the way: the sight gate refused every monster, because a non-player carries no viewshed and the question cannot be asked of it; and `Intent<Use>` is registered by the combat plugin, since the minds may choose an ability in a game that never added abilities.
  Deferred: the targeting cursor and the ability menu, which want the view and panel work; an ability with no user, which belongs to the encounter slice.
  Nate, 2026-09-12: "build the proposed ability system", after asking for one that serves fantasy, pirates, sci-fi, medieval and city crime alike.
- 2026-09-12: abilities, phase D: the targeting cursor and the ability menu, on the finished UI slice.
  `rl-ui` gains `cursor`, the arithmetic the look cursor and the targeting cursor share; `TargetView` and `TargetPanel`, which preview the footprint with the same call the resolver will make and paint it over the map the frame already drew; and `AbilityView` and `AbilityPanel`, which list what the turn-holder knows with the gate's reasons.
  A game writes `AimAt` and the engine does the rest: opens the cursor on the nearest thing the ability's `Aim` wants, or uses a self ability at once, and writes the `Use` on confirm.
  `Offered` now covers whoever holds the turn and lists what was refused and why, so the minds, the menu and the preview read one answer from one gate.
  Knacks lost its stand-in aiming code and gained two tests that drive the cursor through key presses.
  Found on the way: the first capture said "fireball at nothing" over a brute, because the target query required a `Name` and Knacks' monsters had none; the targeting view now counts a target without a `Name` or a `Glyph`, since the ability hits it anyway, and Knacks names its monsters.
  Found after it: a one-row vitals strip drew health and then returned, so every bar a game pushed after it was dropped without a word, though `VitalsView`'s docs invite a game to push one; Knacks had shown its pools as facets to get round it.
  A one-row strip now lays bars out in order with armor, badges, facets and the whereabouts, each drawn whole until the next would not fit, the rule facets already followed; Knacks' pools are bars again.
  Nate, 2026-09-12: "check the rl-ui slice, i think its done. if it is, proceed with phase D."
- 2026-09-12: stealth and awareness, phases A to D of `docs/design/stealth.md`, built on the abilities slice.
  `rl-rules` gains `ai::awareness`: `NoticeStats` and `StealthStats`, the pure `notices` roll against a certain radius and a chance beyond it with light as one bonus, and `Awareness`, whose memory decays so losing the trail means searching rather than forgetting on the spot; `Snapshot` gains `last_known` and the tactics gain `SearchLastKnown`.
  `rl-bevy` gains `stealth`: `Notice`, `Stealth`, `Aware`, `Noticed`, `StealthPlugin` and `StealthRunning`, a `DecideSet::Notice` slot before the minds, and `perceivable`, the one line-of-sight oracle both the minds and noticing read.
  `rl-ui` shows it: `Row::aware` on the nearby rail, and `VitalsView::seen`.
  The delve's beasts notice from RON and its player can smother the brand; Corsair's cave dwellers notice and its islands deliberately do not.
  Found on the way: `Notice` brings an `Aware` with it, so a game that authored observers without the plugin got monsters that never noticed anything, which the delve's own test harness caught; a mind that had not seen the player could descend the flow fields toward it anyway; and nothing in the delve could put the brand out.
  Deferred: sneak attack damage (phase E), two-way stealth, squad alerting, noise.
  Nate, 2026-09-12: "Let's add the stealth awareness."
- 2026-09-13: "seen" asks everything that is watching, not only what keeps track.
  Corsair's strip read hidden while a surface cutthroat, which carries no `Notice` and so sees on sight, was cutting the player down: "seen" had asked only the observers that keep an `Aware`.
  `rl_bevy::Watchers` answers who is watching whom by the rule the minds act on, and `VitalsView::seen` and `Row::aware` both read it, so the rail marks the monster that has seen you whether or not it was ever authored to notice.
  Reproduced first as a Corsair test through the real wiring.
  Nate, 2026-09-13: "corsair shows hidden while the player is being attacked."
- 2026-09-13: two example games, and every mechanic in one of them.
  Corsair is the open world and the delve the dungeon; Lamplight and Knacks are gone, their mechanics moved into the games they fit.
  The delve gains Lamplight's: a brand that burns `Fuel` and will not relight when spent, a torch to carry and set down, a whaler's lamp on every floor, `v` for light as digits; and five knacks from Knacks' fantasy and medieval sets with `Drain` as its own effect, a mana pool drawn as a bar, a whalebone shield the bash asks the slot graph for, gut eels that spit, and a rail showing who has noticed you.
  Corsair gains the pirates set: a broadside on powder, a grapnel cutthroats throw back, a swig of rum, and `Plunder`, which spills a foe's purse at its feet; restored runs keep what the player knows, and now its name and stealth too, which the stealth slice had put only on a fresh player.
  The claim that abilities serve any genre became `crates/rl-bevy/tests/genres.rs`: the five sets load into one registry and build against one set of effects, and a missing effect is refused by name.
  Nate, 2026-09-13: "consolidate the example games. 1 open world and 1 dungeon delving. Add game mechanic examples into those."
- 2026-09-13: one answer to where an ability lands, and one way a turn ends, from the architecture review of the same day.
  `rl-rules` gains `Aim::hits`, `Aim::worth_aiming_at` and `aim_blocked`, the rules for who a footprint catches, what is worth pointing it at and why an aim is refused; `rl-bevy`'s `Bystanders::land` is the one call the resolver lands a use with and the targeting preview previews one with, and the ability tactic scores by the same rules. A property test over seeded layouts holds the preview to the resolver.
  `rl-bevy` gains `Resolution`, every resolver's side of the loop: `claim`, then `done` or `failed`, the one place that knows the player keeps a failed turn and anyone else is charged. The engine's step, wait, warp, item, attack and ability resolvers, the sweep, and the tutorial's shove all use it, and chapter 09 teaches two duties where it taught three.
  `DecideSet::Offer`, `ResolveSet::Travel` and `CleanupSet::{Remove, Requeue}` replace every `.after` and `.before` that named another module's function, which section 3.11 had ruled out; the dead are buried in `Last`.
  Found on the way, by reproducing the review's findings in Knacks before touching them: a spray aimed at allies passed its hurt user by though `Aim::Ally` includes the user; a fireball at the caster's feet read as refused in the banner and still cost twelve mana; the preview listed blockers the resolver never hit; the tactic counted allies as harmed by a foe-aimed burst that never touches them; and no heal had ever landed, because `resolve` clamped every hit at zero and a mend is a negative one. The clamp moved to where blows are rolled, and a heal no longer wakes a sleeping observer the way a blow does.
  Then the first of the review's on-ramp items: `app.needs::<R>(plugin, hint)` replaces the `needs` system, and entering play checks every plugin's requirements together, naming each missing piece with how to make it, where a game used to fix its setup one crash at a time. `CorePlugin` needs its `WorldMap` by name instead of silently skipping every frame without one, which one facts test had been relying on. Every `depends_on` runs in `finish` and every plugin that declares a modal makes the stack itself, so the order of `add_plugins` stops mattering; `MapViewPlugin` needs its `MapView` and field of view; a world built while play never began is warned about; and `Registry::expect` names the registry it looked in.
  Nate, 2026-09-13: "Abilities should be done now, proceed."
- 2026-09-13: the front door, the second of the review's on-ramp items.
  `rl-engine` gains `RoguelikePlugins::new(title, cols, rows)`, a Bevy plugin group holding what every one of the fifteen example programs had typed out by hand: the window sized to the glyph terminal with nearest-pixel sampling, `TerminalPlugin`, `CorePlugin`, `FovPlugin`, the map view, `UiPlugin` and `CapturePlugin`, with `.cell`, `.font` and `.map` for what varied. No subsystem is in it, so opt-in holds, and anything in it can be replaced or disabled as in any Bevy group. The name `EnginePlugins` was not reused, since that was the group `f5d607f` removed for turning every subsystem on.
  `MapViewPlugin::new(rect)` takes its rectangle the way every panel does, so a game no longer inserts a `MapView` of its own; the requirement for one is gone with it.
  Every example and tutorial step starts from the group, which took thirteen lines of window setup and two plugin lines out of each; chapter 1 explains what is inside it and chapter 3 gives the map its rectangle through `.map`.
  Nate, 2026-09-13: "commit then continue".
- 2026-09-13: actors that complete themselves, the third of the review's on-ramp items.
  An actor spawned without `Afflicted` and `StatBlock` silently took no status, and one spawned with `Grants` but no `Known` silently knew nothing; the examples spawned `StatBlock::default()` eighteen times, `Afflicted::default()` twelve and `Speed(100)` thirty-eight to stay clear of it.
  `Actor` now requires `Speed`, which defaults to normal. `StatusPlugin` registers `Afflicted` and `StatBlock` as required by every actor, and `AbilitiesPlugin` registers `Known`, `Pools` and `Cooldowns`, so the components arrive with the subsystem and a game without it carries none, which keeps opt-in. The ability state is required by the actor rather than by `Grants`, because an item that lends an ability is no actor and must not come to know it.
  Every hand-spawned default is gone from the examples and the engine's tests; the status test's player now spawns with neither component and must still take its statuses, and a monster given `Grants` alone must still reach for its ability.
- 2026-09-13: one test stage, which section 3.11 promised as a `TestApp` builder and which was never built; the copies grew instead.
  `rl-bevy` gains `testing`: `TestWorld`, land and sea with one town, floor on the land and wall on the sea; `surface` and `surface_with`, which install it and hand back open ground; `two_sides`, combat rules for two sides at war; and `KeyScriptPlugin` with `press`, keys played after Bevy clears them. Public, so a game's tests and the engine's other crates use the same copy.
  Nine test modules had each defined the world, in two variants that differed only in whether the sea was walled, and three had each written the key player; seven rl-bevy modules, rl-save, rl-ui's harness, rl-overworld and Knacks now use the one module, and the variants are one world whose sea is walled, which every existing test was checked against.
- 2026-09-13: ability state in the save, promised by `docs/design/abilities.md` section 7 and not built.
  A run saved and continued lost every pool, cooldown and charge: the round trip the design asked for, written first, restored a pool as nothing where it had held twelve.
  `EngineSave` gains `abilities`, an `AbilityState` of pools, cooldowns and charges for each entity the game bound, read before the queue hands out ids for actors the game never saved; restoring puts them back on the rebound entities. `EntityRemap::bound` lists what the map holds in id order, so the save is the same bytes for the same run. Older saves load with the field empty.
- 2026-09-13: one lookup for every content file, and content that names content by name.
  `Lookup` was a trait every game implemented, identically, over the same five registries: six copies, in Corsair, the delve, the genres test and three engine test modules. And statuses and affixes were never loaded by name at all: `StatusDef` held raw `u32` ids, so Corsair kept `StatusRon` and `AffixRon`, a hundred lines of mirror types, validation and conversion, which is the pattern the abilities design called out and then left standing beside it.
  `rl-rules` gains `names`: `Names`, a concrete value borrowing whichever of the stat, status, tag, slot and damage kind registries a game has, whose lookups return the message an author reads. `status::load` and `affix::load` join `ability::load`; `StatusDef`, `AffixDef`, `Scaled` and `ScaledStrike` hold typed ids and are no longer deserialized as they stand, since a number in a content file is a modifier that lands on another stat the day the stat list is reordered. `FromArgs` takes `&Names`, and `Abilities::load` builds a file in one call.
  Corsair's mirror types are gone and its files are unchanged; the delve and the tests lost their lookups.
- 2026-09-13: one cursor behaviour, which the abilities slice had split into shared arithmetic and two copies of the handling.
  `move_cursor` and `aim_cursor` each read close, next and a step themselves, off `InspectKeys` and `TargetKeys`, two resources binding Escape and Tab twice, so a game moving "next" had to move it in two places and a fix to one cursor's edge handling was not a fix to the other's.
  `rl-ui`'s `cursor` gains `CursorKeys`, the one set of bindings, and `steer`, the one reading of a frame's keys onto a cell: close over confirm over moving, candidates worked out only when "next" asks for them. Both cursors call it and keep only what they are for: the look cursor its toggle, the targeting cursor the `Use` it writes on confirm. `steer` is tested without an `App`.
- 2026-09-13: combat stands alone, and abilities are built on it rather than into it.
  `combat.rs` held three things: health and the damage pipeline, the minds, and the `Harm` and `Mend` effects. Through the last two it reached into abilities in ten places, reading `Offered`, writing `Intent<Use>`, registering that message and implementing `Effect`, and `status.rs` did the same for `Inflict` and `Cleanse`; so a game with melee and no abilities still built its combat on them, and reading combat meant learning abilities first.
  `rl-bevy` gains `minds` and `effects`. `minds` holds `Mind`, `Perception`, `Profile`, `FlowFields`, `perceivable` and `decide_minds` behind `MindsPlugin`, the one place every action a monster can choose meets, which depends on combat and the field of view; `CombatPlugin` no longer needs the field of view, which only the minds ever read. `effects` holds the seven engine effects and `AddEngineEffects`, and depends on combat and statuses, neither of which now names an ability.
  `MindsPlugin` is opt-in like every other subsystem, and every game and tutorial step that fields monsters adds it beside `CombatPlugin`. Because a forgotten plugin here would look like monsters that never move, a `Mind` spawned without it logs one error saying which plugin to add; section 3.11's rule that a missing piece is reported loudly.
- 2026-09-14: the prelude keeps to what a game writes.
  The Bevy layer's prelude exported `Shove`, `Pull` and `Teleport`, which a game names only in RON, and `Aimed`, `Landed`, `Bystanders`, `Offered`, `FlowFields`, `StealthRunning`, `WindowView` and `PlaceMap`, which only other engine crates use; no example, tutorial step or guide chapter named any of them, and the tutorial's own `Shove` action silently shadowed the engine's. They stay at the crate root, `rl-ui` imports the four it builds on by name, and the prelude's doc says what belongs in it.
  Found on the way: the delve's key hints were pushed to the log at startup and the first floor's name only when the warp landed a frame later, so the log read the keys before the place they were for. The hints now follow the starting floor's name, once, and a test holds the order.
- 2026-09-14: a template to start a game from.
  The on-ramp was the guide, which starts from an empty map and reaches combat in chapter five, and the examples, which are finished games to read rather than start from; a new game began by copying one and deleting most of it.
  `templates/starter` is a `cargo generate` template scoped to the first playable thing: map generation, a player and goblins moving, combat, sight, light and stealth, in one file ordered the way a run happens, with a property test over forty-eight seeds and two tests that play keys through the real input. It carries its own `rustfmt.toml` at the defaults, because a new game formats the way Rust does rather than the way this workspace does.
  `scripts/check-template.sh` fills the two placeholders itself rather than needing cargo-generate, points the engine at the checkout, and runs formatting, clippy and the tests; CI runs it. The workspace excludes `templates`, and no line of the source carries a placeholder whose length would change how it formats.
  Found on the way, generating from GitHub: a game that depends on the engine from git makes Cargo read every `Cargo.toml` in the repository, and the template's placeholder name printed an error in every build of every game. The manifest is `Cargo.toml.liquid`, which cargo-generate renames as it renders, and the check refuses a raw one under `templates`.
  The check first built the rendered game in a target directory of its own, a second Bevy of fifteen gigabytes that filled a disk; it builds in the workspace's, which the same lockfile and profile let it share, compiling only the engine's crates and the game.
- 2026-09-14: the engine owns the run's seed.
  Every game defined its own `Seed` resource, parsed `--seed` itself, and inserted `CombatRng::for_run` and `AbilityRng::for_run` by hand: ten tutorial steps, both examples, the template and three test modules, with a startup panic for the one forgotten, another line in every game for each subsystem that learned to roll, and a save that had to be handed the seed.
  `rl-bevy` gains `seed`: `Seed`, the resource a game inserts once, `Seed::from_args` for `--seed`, and `Seed::stream(name, index)` for a game's own named draws; `Stream` and `add_stream`, by which a plugin derives its stream from the seed before the first turn and again whenever the seed changes, so continuing a save is setting `Seed` and nothing else. `CombatPlugin` and `AbilitiesPlugin` need the seed rather than their streams; `EngineSave::capture` reads it.
- 2026-09-14: the UI base owns the log, and tones and screens are declared in one call.
  `LogPanel` created the `MessageLog`, so every headless test that ran a game's narration had to create it too, three copies and counting; declaring a tone took two lines reaching into the world for `Tones` and then `Palette`; and declaring a screen was a scoped borrow in Corsair and the same `init_resource` and `declare` line in five engine plugins.
  `UiPlugin` inits the `MessageLog`, and a headless test adds `UiPlugin` rather than the log. `AddTone::add_tone(name, colour)` and `AddModal::add_modal(name)` declare while the app is built and make sure their resources exist, so the order a game adds plugins in still does not matter; the engine's own screens declare through the same call.
- 2026-09-14: a game's own content names content the way the engine's does.
  Corsair's monsters and items held damage kinds, factions, slots, tags, drops, statuses and abilities as strings: checked once in a `validate` pass, then looked up again with `expect` at every spawn and every gear refresh, with a panic waiting wherever the two disagreed; the drops were checked in `main`, and the abilities resolved in a second pass after the abilities loaded.
  `Names` becomes one lookup over any registry, the engine's by name and a game's own with `with`, and `rl-rules` gains `NameRef<T>`, a field that is written as a name and holds an id, read only through `Names::load`, which parses each entry on its own so every unknown name in a file is reported at once under the entry it is in. Named `NameRef` rather than `Ref` because Bevy's prelude has a `Ref`, and the facade's prelude test now names it.
- 2026-09-14: one registries resource, and no copies.
  The engine read stats from `StatRules`, statuses from `StatusRules`, slots from `Slots` and damage kinds from `CombatRules`, and every game kept its own tables and cloned each into its resource: the delve's `Rules` and Corsair's `Armory` and `Bestiary` each held registries the engine held again, so a reader had to know which copy to read.
  `rl-bevy` gains `Registries`: damage kinds, factions, stats, statuses, tags and slots, filled once, with `names()` for loading against them. `StatRules`, `StatusRules` and `Slots` are gone and `CombatRules` keeps only the faction matrix. Corsair's new `rules` module loads everything in the order it names itself, vocabulary first and the monsters last, its definitions hold `NameRef`s, and `Armory` and `Bestiary` keep no registries; the delve's `Rules` became its `Registries`.
- 2026-09-14: combat rules are built by naming the pairs.
  Every game set up hostility the same four lines at a time: a `Factions` matrix, `set_mutual` with `Relation::Hostile` per pair, then `CombatRules { factions }`, and `FactionDef { name: "you".into() }` per side; Corsair's took a dozen lines for five grudges and a one-way one.
  `CombatRules::new(&sides)` starts every side neutral, and `hostile`, `hunts` and `allied` name what is not, by id, so a typo is a compile error rather than a panic; `FactionDef::new` names a side. Every tutorial step, both examples, the template and the engine's tests build their rules this way.
- 2026-09-14: the guide teaches names in content files.
  Chapter 8 taught `Registry::from_ron_str` over a `RatDef` that named nothing, so a reader met the engine's loading only in the form that cannot resolve a name, and the first game to need one would have written the check-by-hand pattern Corsair has just dropped.
  The rats now name the damage kind they deal as a `NameRef<DamageKind>`, the root adder's `venom`; steps 8 to 10 load the bestiary through `Registries::names()` and keep no `bite` id of their own, and the chapter shows the load, the error a misspelt kind gives, and `with` for a game's own registries.
- 2026-09-14: Corsair's tasks name their subjects the way the rest of its content does.
  An objective was `(kind: "killed", subject: "cutthroat")`: two strings, matched in a closure to decide which registry the subject was in, parsed as a number for a cave level, compared against "port" and "cove" by hand, and validated in one pass and resolved again in a second.
  `On` is a variant per fact, `Killed("cutthroat")`, `EnteredCave(2)`, `EnteredSite(Port)`, each naming its subject as a `NameRef` to a monster, a side or an item, or as a number or a site, so a subject of the wrong kind is a parse error and an unknown name is reported by the load under the task. `after` names tasks in the same file, the one reference still checked by hand.
- Next: the rest of the deferred pieces (throwing, a character sheet, `TileField<T>`, nights on Corsair's surface, scripted encounters, the unload bridge, and phase H of `docs/design/ui.md`: Bevy UI presenters over the panel views, deferred until a game wants wrapping, hover or sub-cell bars), then the living-world-rogue conversion once the engine is done (Nate, 2026-09-10).
  That conversion keeps its overworld token movement, so `rl-overworld` regains travel on the map alongside the portal picker, and its maps stream as chunks.

Inputs: four Opus code reviews of the three source repos, kept beside this file.

- `roguelike_engine.md` - the prior extraction attempt (~15.7k LOC, Bevy 0.17).
- `living_world_rogue.md` - the overworld game with the Bevy-free `lwr-world` crate (~17.6k LOC, Bevy 0.19).
- `fantasy_rogue_core.md` - this repo's core, map, render, save and infra (~30k LOC).
- `fantasy_rogue_game.md` - this repo's combat, actors, items, ui and assets (~62k LOC).

Every claim below is backed by a `path:line` citation in one of those reports.
This document only synthesises and decides.

Decisions Nate has already made:

- The engine repo is `rl-engine`.
- The first consumer is a brand-new game, **WildReach**, not a port.
  It is a Sunlorn-like: ASCII, one continuous open world you walk across at a single scale, loot, dungeons, encounters, quests, town portals, and possibly a generated victory condition.
  The world is larger than 1024x1024 tiles.
  Content is added incrementally, so the engine's crates and milestones are ordered by what WildReach needs first.
- The second consumer is a small pirate-themed example game, **Corsair**, that lives in the `rl-engine` workspace.
- The overworld stays as a picture of how rivers, biomes and climate fit together, and as the place to pick a portal destination.
  It is an opt-in crate so WildReach can drop it later without touching anything else.
- Travelling far means walking there once to discover it, then using a portal to return instantly.
  Travel does not simulate time.
  There is no route-travel command.
- Rivers are in scope.
- No more questions for now; build, and course-correct later.
- `living-world-rogue` is source material for world generation, not the integration target.
- Randomness uses `rand`.
  Cross-version stream stability is not a goal because the game will evolve and break determinism anyway.
- GOAP is not ported.
- `rl-ui` is built in from the start.

## 1. The one lesson that matters most

`roguelike_engine` was extracted, published, tested (457 tests, 0 failures) and then not used.
This repo does not depend on it.
`src/core/turns.rs` holds a byte-level copy of its `TurnManager`, plus the ~900 lines of turn-phase machinery the engine never shipped.

The failure was not code quality.
The engine shipped data structures and left the behaviour in the game.
It shipped `TurnManager` but not the turn loop, `MonsterAI` knobs but not the execute step, `AbilityDef` but no resolution, FOV but nothing that consumed it.
Its own example runs ten turns in which nothing moves, fights or sees.
Adopting it solved none of the hard problems, so the game copied the easy parts and moved on.

Rule for `rl-engine`: **for every subsystem, either own the loop or leave the subsystem out.**
A struct plus a `SystemSet` marker is not a subsystem.

Corollary: the engine is developed against a real game from the first milestone.
Every milestone in section 8 is defined by a playable slice of the new game, and the engine ships only what that slice pulls on.

## 2. What the reviews agreed on

All four reviewers converged independently on these points.

1. **Themes live in the game.**
   The engine is lexically theme-free.
   No fantasy word appears in an engine crate.
2. **`#[non_exhaustive]` + `Custom { id }` does not work and must not appear.**
   In `roguelike_engine` every custom tile is inert: not walkable, not opaque, not flammable, named `"Custom"`.
   The compiler even flags the defensive `_ =>` arms as unreachable inside the defining crate.
   `Custom` works only where the enum carries no behaviour, which is exactly where an enum was not needed.
3. **Closed enums do not work either.**
   This repo has thirteen closed taxonomy enums (`DamageType`, `StatusEffect`, `EquipEffect`, `ItemCategory`, `EquipSlot`, `Faction`, `TacticId`, ...).
   `lwr-world` has eight exhaustive tables over `Biome`.
   A sci-fi game cannot add `Radiation` or `Nebula` without forking.
4. **The seam that worked, three times by accident, is a registry keyed by an opaque id plus a trait for behaviour.**
   Prefabs, monsters and props in this repo already do this, and exactly one hard-coded content string survives in 62k lines.
5. **The Bevy-free core is the right spine.**
   `lwr-world` proves it: one dependency, 249 tests in 4 seconds, no window.
   The compiler must hold that line, because conventions did not: this repo's `core/turns.rs` imports combat, items and player.
6. **The renderer must not be one text entity per tile.**
   Both games do this (5,760 and 13,200 entities).
7. **Neither game has benchmarks, and the engine's benches measure the wrong things.**
   Six of eleven are under 4 ns; FOV, pathfinding, lighting and choke maps are unmeasured.

## 3. Decisions

### 3.1 Themes: out of the engine

Mechanics are engine.
Vocabulary is game.
Lighting, factions, stealth, statuses, tile fields and quest triggers are mechanics and belong in the engine.
Which factions, statuses or tiles exist is the game's business.

The engine's `examples/` carry a small shared content module (a dozen tiles, a few monsters, a few items) so every example is a real playable demo.
That is what `roguelike_engine`'s example lacked.
The full worked theme is the new game itself.
A second, deliberately different consumer (sci-fi or pirate) is the only real proof of theme-agnosticism and is scheduled last.

### 3.2 Extension mechanism: registries and traits, never enum variants

Two mechanisms, chosen by whether the extension is data or code.

**Data extensions use an opaque id and a registry.**
`TileId(u16)` plus `TileRegistry` holding `TileProps { walkable, passable, opaque, obstacle, move_cost, flammability, promotion, glyph }`.
Same shape for `DamageTypeId`, `StatusId`, `StatId`, `SlotId`, `FactionId`, item tags and seed domains.
Lookup is an array index, which is faster than the `matches!` chains it replaces.
Games register from RON at startup.
The engine ships a `standard()` set (wall, floor, door, stairs) for convenience.
The compile-time exhaustiveness that closed enums gave is recovered by a `validate()` hook on registry load plus guard tests over the live assets, which this repo already practises.

**Code extensions use traits taken as parameters.**
`Pass<C>`, `BuildContext`, `OpacitySource`, `CostSource`, `MapOverlay`, `Tactic`, `DamageStage`, `Narrator`, `ThreatSubject`, `ExploreInterrupt`, `SaveBackend`, `Renderable`, and `rand::Rng` for randomness.
The best existing examples are `dequeue_next_batch_pure(_, is_player: impl Fn(Entity) -> bool)`, `accumulate_equip_effects(items: impl Iterator<...>)`, `trace_shot(hostile_at: impl Fn(i32, i32) -> Option<Entity>)` and `TurnQueue::pop_due(is_alive: impl Fn(Entity) -> bool)`.
Each removes a Bevy dependency by asking the caller for the capability.

Generic type parameters are reserved for containers (`Grid<T>`, `TileField<T>`) and the builder context (`Pass<C>`).
Making `Map<T: TileSemantics>` generic was considered and rejected: it infects every downstream type and blocks data-driven tiles.

### 3.3 Randomness: `rand`, named streams, per-run root, cosmetic split

Generator: `rand` 0.9.
Gameplay and world streams use `StdRng` (ChaCha12), not `SmallRng`.
`SmallRng` picks Xoshiro256++ on 64-bit and Xoshiro128++ on 32-bit, so a world seed would generate different terrain on native and on wasm32.
`StdRng` is the same algorithm on both.
Cosmetic streams may use `SmallRng`.

Derivation keeps what worked in the two games, which is about stream isolation, not about the generator:

- `RunSeed` rolled once per run, saved, shown on the outcome screens.
- `RunSeed::derive(domain, index) -> u64` folding a domain salt and an index (floor, chunk, pass) through a mixer, then `StdRng::seed_from_u64`.
  From this repo's `src/core/rng.rs`.
- Domains are open constants, `SeedDomain::new(b"WEATHER")`, so a game adds a stream without editing the engine.
  From `lwr-world`'s string-keyed `stage_seed`.
- Every pass in a generation chain gets its own stream keyed by its name, so inserting a pass cannot reroll its neighbours.
  From `lwr-world`'s `Chain`.
- `FxRng` for cosmetic randomness, declared outside the determinism contract so FX systems need no ordering edges.
  From this repo.
- Position-derived draws where iteration order could leak, as in `lwr-world`'s `jitter_at`.

Functions take `&mut impl Rng`.
There is no engine `RngSource` trait; `rand::Rng` is that trait.
No `with_seed(u64)` on anything; seeds come only from `RunSeed::derive`.

Determinism is still an invariant within one build: same seed, same world, same combat, for replays, bug reports and tests.
It is not promised across engine versions.
The test that guards it is "same seed twice gives byte-equal tiles", which none of the three repos has.
Two live holes get fixed on the way in: `roguelike_engine`'s `randomize_grid` uses `rand::rng()` so every lake differs at a fixed seed, and this repo's `FireRng` is seeded from a constant and never reseeded.

### 3.4 Map generation: `lwr`'s pass rigour on `roguelike_engine`'s context trait

Base the pipeline on `lwr-world/src/local/chain.rs`, not on `BuilderChain`.
`Pass::name()` keys the pass's RNG stream.
Duplicate names are rejected.
`phase()` is mandatory and there is no unchecked escape hatch.

Take from `roguelike_engine` the `BuildContext` trait so passes are generic over a game-extended context, the `take_snapshot` hook, `FloorProfile`, and the `BuilderPhase` ordering.
Take from this repo the wasm-safe timing and the removal of the unseeded constructor.

Add what none of them have:

- `apply` returns `Result<(), BuildError>` so a chain can retry.
- `Chain` is `Send` so generation can move off the main thread.
- A typed output seam, `ctx.emit::<T>(value)`, so `PrefabStamper` can drop its `Arc<Mutex<Vec<..>>>` side channel.
- Rooms are optional by design, not `Option<&Vec<Rect>>` forced on cave builders.

One `Chain` type serves both world passes (terrain, vegetation, settlements, roads) and dungeon builders (room accretion, BSP, caves, cullers, doors, stairs, decoration, prefabs, choke map).
Fix the three complexity classes when porting: incremental door sites in room accretion (currently 2,000 attempts each rescanning the map), worklist pruning in the choke map (currently quadratic in map area), and single-pass region labelling (currently one flood per unlabelled cell).

### 3.5 The overworld: what to drop and what to keep

Nate's instinct is against an overworld.
The thing worth being against is the *mode switch*: a zoomed-out screen where the player is a token, and local maps that are boxes with edges you enter and leave.
`living-world-rogue` has that model and it is the part that feels like a menu.
Sunlorn does not, and that is the feel WildReach wants.

But "overworld" names two different things, and only one of them is the mode switch.
The other is a coarse world structure: where the biomes, sites and roads are, decided before any tile exists.
Every open-world roguelike that is bigger than a few screens has that structure whether or not it shows it.
Cataclysm DDA presents one continuous map and still keeps an overmap underneath it plus a zoomed view of it on a key.
Dwarf Fortress adventure mode is the purest continuous world in the genre, and it is unplayable without its travel mode.
Sunlorn's towns and roads did not place themselves at tile scale either.

A world larger than 1024x1024 settles the question.

- **Generation.**
  `lwr-world` generates 240k cells in about a second.
  A 4096x4096 world is 16 million tiles, so a full eager generation is a minute or more at new game, before any settlement pass.
  Tiles have to be generated on demand from a structure that was generated eagerly.
  That structure is an overworld.
- **Traversal.**
  Crossing 4096 tiles at one tile per turn is a long walk even with key repeat.
  Sunlorn mitigates this with portals and road travel.
  At this size WildReach needs a travel command, and a travel command plans over the coarse structure.
- **Memory and simulation.**
  Sixteen million tiles fit in memory, but nothing may touch them per turn.
  The active region has to become the set of loaded chunks around the player, which is Cataclysm's reality bubble.

So the recommendation is: **keep the overworld as a data model and as generation structure, and treat any overworld screen as a lens onto that data, never as the player's location.**
Nate wants the lens now, including travelling through it, and wants to be able to remove the travelling later.
That is achievable with one invariant and one crate boundary, spelled out in 3.5.1.
Concretely:

1. **`WorldGraph`**, the coarse layer.
   The world is divided into regions of, say, 64x64 tiles; a 4096x4096 world is a 64x64 grid of regions.
   Each region carries its elevation band, climate, biome band, an optional site, and the roads passing through it.
   This is `lwr-world`'s `Overworld` almost verbatim, and it generates in milliseconds.
   The quantile banding is computed here, once, over the whole world.
2. **Chunks**, the fine layer, generated on demand.
   A chunk is a pure function of the run seed, its region, and its neighbouring regions.
   It samples the same continuous noise the coarse layer sampled, at tile resolution, so it agrees with its region's bands.
   This is `lwr-world`'s local map generation with `Surroundings` and `TileFacts`, made generic over the facts type.
3. **Seamless boundaries.**
   Chunks stitch with no edge.
   Linear features that cross a boundary (roads, rivers, walls) agree on the crossing point through `lwr-world`'s seam hash, which the reviewer called the most reusable idea in that repo.
   Compact features never straddle a boundary: a settlement is placed at its region's centre with a footprint smaller than the region, so buildings, docks and dungeon entrances always belong to exactly one chunk.
   That single constraint is what makes independent chunk generation sound.
4. **Chunk lifecycle.**
   The loaded set is the chunks within the active region.
   A chunk that leaves the region is unloaded; if it was mutated (dug, burned, looted, a door left open) its delta is kept and replayed when it loads again.
   Unmutated chunks are simply regenerated, which is what makes a 4096x4096 world cost nothing to store.
5. **Walking scale is the ground truth.**
   The player's position is always a world tile coordinate.
   Walking off the edge of a chunk streams the next chunk in; it never returns the player to an overworld screen, which is the `lwr` behaviour WildReach does not want.
   Town portals teleport between discovered sites.
6. **Dungeons** are separate maps on a map stack behind entrances, as before.

Everything Nate liked about Sunlorn survives: no boxes, the whole world walkable at one scale.
Everything `lwr-world` got right survives too: deterministic worlds, fast generation, testable passes, and the world picture.

World size target: 4096x4096 tiles as the design point, with nothing in the architecture that depends on it.
Region size is a config field, not a constant, because `lwr-world` sized its local maps to the terminal and regretted it.

#### 3.5.1 The overworld screen: use it now, drop it later

The overworld screen has two jobs, kept separate so either can be switched off.

| Job | Where it lives | WildReach |
|---|---|---|
| **View**: render `WorldGraph` (bands, rivers, sites, roads, discovered regions) as a map | `rl-render`, a `Renderable` over regions | always |
| **Portal picker**: choose a discovered site and portal to it | `rl-overworld`, opt-in | now; droppable |

The invariant that makes the second row removable: **the overworld never owns the player's position.**
In `lwr` the overworld cell is the location and entering it builds a local map.
Here the marker on the overworld is derived from the tile position divided by the region size.
Choosing a discovered site writes a portal request; the portal mechanic in `rl-bevy` moves the player and streams chunks.
Undiscovered regions cannot be chosen, because getting somewhere the first time means walking there.

Because `rl-overworld` only reads `WorldGraph` and the discovery set and writes a portal request, nothing else depends on it.
Removing it from WildReach's `Cargo.toml` leaves the view and the portal mechanic working unchanged.
`lwr`'s `Scale` / `Travel` state machine is not ported; the overworld is a screen, never a mode the player moves in.

#### 3.5.2 Travel

Far travel is walking.
The first visit to a place is always on foot, through whatever lies between.
Once a site is discovered, a portal takes the player there instantly, with no time simulated and no encounter rolled.
Auto-explore and travel-to-a-seen-tile within the active region are conveniences over walking, and they do simulate every step, because they are walking.
There is no cross-world route command in either form.

#### 3.5.3 Rivers

Rivers are the second linear feature after roads, and the reason the seam primitive is designed as a general `LinearFeature` rather than a road special case.

- **Hydrology pass on `WorldGraph`**, between elevation and climate.
  Sources are regions above an elevation quantile with enough moisture.
  Each river follows steepest descent across the region grid, merging where paths meet, pooling into a lake at a local minimum, and ending at the sea.
  Flow accumulation gives each segment a width class.
  This is the same shape as `lwr-world`'s road network: a few hundred lines over the coarse grid, deterministic, testable with seed-range properties ("every river reaches sea or lake", "no river flows uphill").
- **Rivers feed the rest of the coarse layer.**
  `lwr-world`'s `distance_to_water` becomes distance to sea or river, so moisture and biome bands respond to rivers.
  Site scoring prefers river banks.
  The road router prices a river crossing so roads bridge at narrow points.
- **Per region, a river is a `LinearFeature { kind, enters: (edge, offset), exits: (edge, offset), width }`.**
  Offsets on shared edges come from the seam hash, so two neighbouring chunks agree where the river crosses without negotiating.
  A chunk draws its channel between its entry and exit points with the given width, and a bridge tile where a road's crossing coincides.
- **At walking scale** a river is water tiles from the registry with a `CostSource` that makes them impassable to land walkers, fordable at marked shallows, and navigable for the sailing `MovementProfile`.
  That last point is what Corsair will lean on.

Hydrology lands in M1 so the overworld view shows a coherent world from the first build.
River channels and bridges in chunks land in M4 with roads and settlements.

### 3.6 Turn scheduling: the engine owns the loop

The pure heap is identical in all three repos and comes across as-is, generic over `Id: Copy + Eq`.
Take `lwr`'s hand-written `PartialEq` matching `Ord`, its `pop_due(is_alive)` signature, and its argument for an integer clock.
Take this repo's `is_alive` guard and `MAX_NPC_BATCH` of 64.

Then lift this repo's `ProcessingPhase` chain (Brain, ResolveMovement, ResolveActions, Cleanup) and `TurnState` gate into the Bevy crate.
This is the machinery `roguelike_engine` left behind and the reason it was not adopted.
Three changes on the way:

- The scheduler advances the clock; games stop hand-rolling it.
- Death resolution is specified to run before requeue, so the two duplicated defensive checks become one contract.
- Speed comes from a `SpeedModifier` trait or an `ActionCost` message, not from reading `Chilled`, `Hasted`, `Slowed` and `Berserking` by name.

Open world addition: an **active region**.
The active region is the set of loaded chunks around the player.
Only actors inside it are scheduled.
Actors outside it are frozen with their chunk, or advanced coarsely when the region reaches them.
This is what keeps a sixteen-million-tile world at turn-based cost.

### 3.7 Pathfinding: Dijkstra maps per pather class, A* for unique goals

A Dijkstra map is a grid of integers where each cell holds its cost to the nearest goal cell.
It is built by one search that starts from every goal at once, so the whole map costs one pass regardless of how many goals or how many consumers there are.
A monster uses it by looking at its eight neighbours and stepping onto the lowest value.
That step is eight array reads.

The contrast with what this repo does today: every hunting monster runs its own A* per turn toward the same player.
Fifty monsters means fifty searches, each evaluating the hazard cost function along its own frontier.
One Dijkstra map rooted at the player answers all fifty.

**Why one map per pather class.**
The values in a map depend on the cost function, and the cost function depends on what the mover can do.
A monster that opens doors sees a different map from one that cannot.
A swimmer, a flier, and something immune to fire each see different maps.
Two monsters with the same capabilities see the same map and share it.
So the engine interns a `MovementProfile` (bitflags: opens_doors, swims, flies, fire_immune, avoids_webs, ...) to a small `ProfileId`, and keeps one map per `(goal set, ProfileId)`.
A typical map has two to four distinct profiles among the monsters that are awake, so it is two to four floods per turn instead of fifty searches.

**Goal sets.**
"Toward the player" is one map.
Others come free from the same machinery: toward the nearest item of a kind (fetch), toward the nearest exit, toward allies, toward the last heard noise.
Sound propagation with decay is literally a Dijkstra map, which is what this repo's noise system built by hand.
`lwr-world`'s `distance_to_water` and `distance_field` are two hand-written copies of the same multi-source flood.

**Fleeing.**
Take the player map, multiply every value by a negative factor slightly above one (Brogue uses -1.2), and rescan from those values as the starting costs.
The result is a safety map whose low points are far from the player *and* reachable without passing near the player.
Fleeing monsters descend it.
It costs one extra pass and replaces the corner-walking heuristics in `flee_direction`.

**Invalidation.**
The player map is recomputed when the player moves, which is most turns.
A profile's map is also dirty when its cost field changed: a door opened, fire spread, a web was cut.
Those arrive through the map mutation messages.
Maps are built lazily on the first monster request in a turn, so a turn with nothing awake costs nothing.

**On a continuous open world.**
The flood must not visit a million cells per profile per turn.
Bound it to the active region from section 3.6, or stop expanding at a maximum distance.
Cells outside are unreachable, which is correct because actors outside are not scheduled.
Use a bucket queue (Dial's algorithm) since step costs are small integers, giving linear time in cells visited rather than log-factor heap time.
Store `u16` per cell with `u16::MAX` meaning unreached.
Keep one reusable scratch buffer per profile with a generation counter, so a rebuild neither allocates nor clears.

**Descending.**
Pick the minimum neighbour with a fixed neighbour order for deterministic ties.
Occupancy is not in the map, so a step onto an occupied cell falls back to the next-lowest neighbour or waits, which is what produces natural queueing in corridors rather than conga lines.
Apply the diagonal-corner rule this repo's `is_frontier` already gets right.

**Where A* stays.**
A single mover with a unique, distant goal: a courier walking to a named town, a quest NPC heading home, the player's travel-to command.
Those keep A* with this repo's `PathCache`, which probes the cached path before searching again.
The rule is: shared goal, Dijkstra map; unique goal, A*.

**The API.**
One type, `DijkstraMap`, with `build(goals, cost: impl Fn(idx) -> Option<u16>, bound)`, `descend(idx) -> Option<idx>`, `rescan_scaled(factor)`, and `value(idx)`.
It serves AI, sound, auto-explore, stair placement, site scoring and flee maps.
It lives in `rl-grid` with a benchmark at three active-region sizes.

### 3.8 Drop bracket-lib; own the algorithms

The fork is narrow in use (Point, Rect, FOV, A*, Dijkstra, Bresenham, dice) and carries three small patches over an upstream that is no longer maintained.
Its FOV returns a `HashSet<Point>`, which is the hottest allocation in the game and a hash-iteration-order hazard.
Its `DijkstraMap` does not zero seed depths, which this repo works around in auto-explore, and it has no bound, no rescan and no bucket queue.

The engine owns: `Point`, `Rect`, `Direction`, `DirectionSet`, Bresenham, symmetric shadowcasting FOV writing into a bitset, A* with an optional entry-direction state and turn cost (from `lwr`'s road router), `DijkstraMap` as above, and a pre-parsed `Dice` struct.
That is roughly 1,500 lines, all benchmarkable, all tested against the current outputs before bracket-lib is removed.

`petgraph` is also dropped.
`roguelike_engine` uses it once, to hold a tree of rectangles.
`lwr`'s road network needs a `DisjointSet` and a Dijkstra over sites, which are 130 lines.

### 3.9 Data layout for performance

- **Three resources, not one `Map`.**
  `Terrain` (per map, rarely mutated), `Occupancy` (per frame, a spatial index), `Knowledge` (per viewer).
  This repo rebuilds `blocked` every frame through `ResMut<Map>`, which marks the whole resource changed and silently defeats three full-map change-detection gates at 60 Hz, including a `DefaultHasher` pass over every tile.
- **`SpatialGrid`** with `at(x, y) -> &[Entity]` and `in_radius`, updated on `Changed<Position>`.
  This repo has no entity-at-tile index; every AoE, cleave, explosion and occupancy check is a scan over all actors.
- **Bitset viewsheds**, a word per 64 tiles, deterministic iteration.
- **`DijkstraMap` per pather class** per section 3.7.
- **`TileField<T>`** with double-buffered integer kernels, no allocation in the tick, RNG-free by default.
  Fire, gas, sound, scent, heat and blood are one type with different kernels.
  This repo's `gas.rs` is the reference implementation.
  Fields are bounded to the active region on a large map.
- **Sparse active sets** for tile promotion instead of a full-map scan per turn.
- **Reusable scratch buffers** in the router, keyed by a dirty list.
  `lwr`'s router allocates 136 bytes per cell per call, which is the whole of its 1.1 s at 600x400.
- **Interned `Id<T>(u32)`** for content references.
  `Cooldowns(HashMap<String, u32>)` puts string hashing inside the AI's double loop.
- **No `HashMap` or `HashSet` in gameplay paths.**
  `BTreeMap`, `Vec` or bitsets, with the reason stated as `lwr` does.
- **Shadowcast lighting**, O(r^2) per source, and no global viewshed redirty when a light changes.
- **Viewport rendering.**
  The glyph grid draws the visible window, never the world.

Benches from day one, on the real paths: FOV at 64 actors, A* and `DijkstraMap` at three region sizes, lighting at 20 sources, choke map, each builder, a full world chain at three sizes, `TileField` tick, and the render sweep.
`roguelike_engine`'s criterion harness is the starting point; its cases are not.

### 3.10 Rules layer: shapes in the engine, vocabulary in the game

| Engine ships the shape | Game ships the instances |
|---|---|
| `DamageTypeId`, resistance ladder, `ResistAccum` | which damage types exist |
| `StatusDef` registry: duration, stacking, per-turn effect, modifiers, badge | which statuses exist |
| `Modifier { stat: StatId, op: Add / Mul / Compound / Max, value }` and one accumulator | which stats exist |
| Equipment slot graph: occupancy, two-handed claiming, dynamic resolution | which slots exist |
| Hook points: on_hit, on_defend, on_kill, on_death, on_block, damage_mods | what each hook does |
| `TargetMode`, `AoeShape`, footprint resolution, LOS and wall bounding | which abilities exist |
| `DamageEvent` with the `attacker` / `credit` split, and a staged pipeline | the balance numbers |
| `Tactic` trait, `TacticCtx`, the pure decision helpers | which tactics exist |
| `ContentRegistry<T>`, `BandedWeightedTable<T>`, validate-on-load, RON loading | the RON |
| `Narrator` trait over typed log events | the sentences |
| Typed world events and a trigger registry (entered region, killed def, picked up tag, talked to) | which quests exist and what victory means |
| Balance checker over `ThreatSubject` | the threat inputs |
| `FactionId` and a relation matrix | which factions exist |

Specific ports:

- The damage pipeline becomes named stages the game composes (`ScaleByAttacker`, `ApplyResistance`, `TryBlock`, `SubtractArmor`, `ScaleByDefender`, `Commit`, `RunHooks`).
  This repo's single 615-line system at Bevy's 16-parameter ceiling is the thing being replaced.
- Second-pass reactions (thorns, cleave, attacker statuses, splits, blinks, steals) go on one engine `ReactionQueue` instead of five game-owned messages the engine plugin registers.
- `AttackIntent` exists.
  Attacking is not a side effect of walking into someone.
- The affix and enchant model (`AffixDef`, `ScaledOnHit`, `EnhanceRule`) ports nearly as-is, keyed on game-defined tags instead of `ItemCategory`.
- The tactic-priority AI is the only AI.
  GOAP is not ported.
- The log carries a category and a structured payload.
  This repo classifies log lines by matching about thirty English substrings.
- Quests are game content.
  The engine's contribution is that every gameplay outcome is a typed event a game can subscribe to, plus a registry of named triggers and counters, so a quest or a generated victory condition is data over those events.

### 3.11 Bevy layer conventions

- Every engine plugin's systems run in a turn-driven schedule gated by `run_if` by default.
  `roguelike_engine` puts every system in `Update` ungated, so poison ticks sixty times a second and FOV runs in the main menu.
  The `SystemSet` markers stay exposed for reordering.
- The three-owner ordering rule from this repo's `CLAUDE.md` is kept and given enough named phases that no downstream plugin ever writes `.after(concrete_system)`.
- `#[derive(Message)]` with `MessageWriter` and `MessageReader` is the seam between engine and game.
  `roguelike_engine/src/map/mutation.rs` is the model: request messages, engine apply systems doing only data sync, game reactions `.after(MapMutationSet)`.
- A `MapBound(MapId)` marker makes map teardown and the map stack automatic instead of a hand-maintained list of component types.
- A `TestApp` builder replaces plugins registering other modules' messages for test convenience.
- Bevy 0.19, matching `living-world-rogue`.
  The Bevy-free tier is what makes engine version churn survivable.

### 3.12 UI: reused from the start

`rl-render` owns the world view: a `GlyphGrid` drawn as one instanced mesh over a font atlas, a `Renderable` trait with an engine-owned `Cell`, the lit / remembered / occluded rules from this repo, camera follow, particles and screenshot capture.
Both games spawn one text entity per cell; that backing store is rewritten, the rules are kept.

`rl-ui` owns chrome and modals, ported from this repo's `src/ui/`: semantic palette tokens, the list-to-detail widget, key hints and keybinds, the framed modal, the semantic log view, and the side panel skeleton (stat bars, status badges).
The `ActiveModal` closed enum becomes a registry of modal ids the game populates.
The widget must not import its consumer; today `list_detail` re-exports helpers from `inventory_preview`.

Revised 2026-09-12: built, as phases A to F of `docs/design/ui.md`; that document is the reference and section 11b of it records where the build differs from the plan.
Revised 2026-09-11: this section said Bevy UI, and what was built draws on the glyph terminal.
`docs/design/ui.md` settles it, and the answer is that the backend was the wrong thing to decide first.
Every panel splits into a view (the data), a collector (the system that keeps it fresh) and a presenter (one way of drawing it); the views and collectors are the reusable half and do not know which backend draws them.
Terminal presenters ship first because that is what the three examples want and because an exact-text assertion is a better test than a node tree.
Bevy UI presenters come later, over the same views, when a game asks for wrapping, hover or sub-cell bars.
The map view stays a glyph grid in either case, because an ASCII game's map is a glyph grid.

### 3.13 Documentation and test rules

- `#![deny(missing_docs)]` on every crate.
- Doc-tests compile and run.
  All seven in `roguelike_engine` are `ignore`d, which is how its docs came to promise three `Custom` variants that do not exist.
- No blanket `#![allow(clippy::too_many_arguments, clippy::type_complexity)]`.
  Those are the lints that would have flagged the 16-parameter systems.
- House style is `lwr`'s "why and why-not" doc comment, at this repo's density.
- Every RON schema carries a top-of-file comment listing the full option space.
- Test idioms required in the engine and documented for games: property-over-seed-range, fingerprint tripwires labelled as such, `ALL` constants on content tables, headless `App` tests, and guard tests over live assets.
- `examples/` grows one example per milestone, each runnable: headless world dump with snapshots, a walking `@` with FOV, monsters that hunt and fight, loot and equipment, a dungeon entered and left by portal, a save round-trip, the balance report, a custom tile, a custom `Pass`, a custom `Tactic`.

## 4. Crate layout

Boundaries are drawn on dependency weight, not subject.
The load-bearing line is between tier 1 and tier 2.
CI fails if `cargo tree` for any tier-1 crate shows `bevy`.
Crates are listed in the order they come into existence; the milestone column says when.

| Tier | Crate | Contents | Deps | Milestone |
|---|---|---|---|---|
| 0 | `rl-core` | `Grid<T>`, `Grid2D`, `Point`, `Rect`, `Direction`, `DirectionSet`, geometry (Bresenham, cone, disc, distances), `DisjointSet`, `RunSeed` + `SeedDomain` + derive, `Dice`, `Id<T>` interning, quantile stats, `TurnQueue<Id>` and the action cost model | `rand` | M1 |
| 1 | `rl-grid` | `TileId` / `TileRegistry` / `TileProps`, `Terrain`, `MapOverlay`, `OpacitySource`, `CostSource`, flood / label / distance field, shadowcast FOV into bitsets, A* (optional turn cost), `DijkstraMap`, `PathCache`, `SpatialGrid`, `TileField<T>`, tile promotion, lighting | core | M1 |
| 1 | `rl-mapgen` | `Chain` / `Pass<C>` / `Phase` / `BuildContext`, snapshots, decoration rules; later the dungeon builders, prefab stamping, choke map | core, grid | M1 (chain), M4 (dungeons) |
| 1 | `rl-world` | `Fbm`, `SampleSpace`, quantile banding, `WorldGraph` (regions, bands, hydrology, sites, roads), `LinearFeature` and the seam hash, chunk generation from `Surroundings<F>`, scored site placement, road router and network, settlement and vegetation passes, river channels and bridges, chunk deltas | core, grid, mapgen, `noise` | M1 (graph, hydrology, chunks), M4 (settlements, channels) |
| 1 | `rl-test-support` | ASCII map fixtures, seed-range property helpers, neighbourhood fixtures | core, grid | M1 |
| 2 | `rl-bevy` | plugins, `Position` / `Viewshed` / `Collider`, `ProcessingPhase` and `CombatPhase` sets, the turn loop, active region and chunk streaming, occupancy index, map stack and `MapBound`, mutation messages, `ReactionQueue`, `TestApp` | all above, `bevy` | M1 |
| 2 | `rl-render` | `GlyphGrid` instanced renderer, engine-owned `Cell`, `Renderable`, the `WorldGraph` view, camera, particles, screenshot | bevy | M1 |
| 2 | `rl-overworld` | opt-in: overworld screen with a portal picker over discovered sites; reads `WorldGraph` and discovery, writes a portal request, owns nothing else | bevy, world | M1 |
| 2 | `rl-ui` | tone registry and palette, panel views and collectors, terminal presenters, list-detail widget, key hints, framed modal, semantic log view, modal stack | bevy | M1 (shell), M3 (inventory widgets), UI phases in `docs/design/ui.md` |
| 1 | `rl-content` | `ContentRegistry<T>`, `BandedWeightedTable<T>`, validate hook, RON loading, `include_str!` convention | core, `serde`, `ron` | M2 |
| 1 | `rl-rules` | stat / modifier stack, damage stages and `DamageEvent` shape, status registry with generic tick and expire, hook vocabulary, targeting and AoE footprints, factions matrix; later the slot graph and the affix and enchant model | core, grid, content | M2 (combat), M3 (equipment) |
| 1 | `rl-ai` | `Tactic` + registry + `TacticCtx`, `MovementProfile`, pure decisions, ability scorer, stealth and awareness; later the auto-explore and travel pure half | core, grid, rules | M2 |
| 1 | `rl-events` | typed world events, trigger registry, named counters | core, content | M5 |
| 2 | `rl-save` | `SaveBackend`, native and wasm backends, `beforeunload` bridge, `Versioned` policy, `SaveId` remap, chunk delta persistence | core, bevy | M6 |
| 1 | `rl-tools` | balance checker over `ThreatSubject`, seed replay, headless dump | content, rules | M6 |
| 3 | `rl-engine` | facade with a curated prelude, `examples/`, `benches/` | everything | M1 |
| 3 | `corsair` | the pirate example game, a workspace member with its own assets | everything | M7 |

Sixteen crates, of which nine exist after the first milestone.
Revised 2026-09-11: `rl-content`, `rl-events`, `rl-ai` and `rl-tools` are modules of `rl-rules`, and `rl-test-support` is gone; see the progress entry for why.
`rl-grid` may later split into `rl-fov`, `rl-path` and `rl-field` without breaking consumers.

Workspace `Cargo.toml` ships `lwr`'s profile trick: dependencies at `opt-level = 3`, workspace crates at `1` in dev.

## 5. What comes from where

| Piece | Source | Change on the way in |
|---|---|---|
| `RunSeed::derive`, `FxRng`, four determinism tests | `fantasy-rogue/src/core/rng.rs` | `StdRng` behind it, domains become open constants |
| Named per-pass streams | `lwr-world/src/rng.rs`, `local/chain.rs` | keyed through `RunSeed::derive` |
| `Chain` / `Pass` / `Phase` | `lwr-world/src/local/chain.rs` | generic context, `Result`, `Send`, `emit` |
| `BuildContext`, `FloorProfile`, snapshots | `roguelike_engine/src/map/builders/mod.rs` | `impl Rng`, rooms optional |
| Elevation, climate, banding, `Fbm`, `SampleSpace`, stats | `lwr-world/src/{elevation,climate,fbm,sample,stats}.rs` | band-to-tile through a game classifier |
| Site placement, road router and network | `lwr-world/src/{sites,roads}.rs` | `CostSource` instead of `friction(Biome)`, scratch buffers, `PlacementRules` struct |
| `Overworld` as `WorldGraph`, `Surroundings` / `TileFacts`, seam hash | `lwr-world/src/{overworld,local/surroundings,local/passes/finish}.rs` | `Surroundings<F>` generic over facts, seam hash generalised to `LinearFeature`, chunks instead of local maps, hydrology pass added |
| Settlement, vegetation, cavern, ruin passes | `lwr-world/src/local/passes/` | stamp a region of one map, tiles via registry |
| `Grid<T>`, `Direction`, `Room` | `lwr-world/src/{grid,direction,local/room}.rs` | fix `is_empty`, avoid per-cell division in `iter` |
| `Grid2D` trait | `fantasy-rogue/src/map/grid.rs` | as-is |
| Map overlay idiom | `fantasy-rogue` `SmokeOpacityMap`, `HazardMap`, `KnownMap` | formalise as `MapOverlay` with default forwarding |
| Multi-source distance field | `lwr-world` `distance_to_water`, `distance_field`; `fantasy-rogue` auto-explore | one `DijkstraMap` with bound, rescan, bucket queue |
| Dungeon builders, cullers, decoration rules | `fantasy-rogue/src/map/builders/` | `TileId` targets, incremental door sites |
| Choke map, lakes | `roguelike_engine/src/map/builders/` | worklist pruning, seeded blobs |
| Prefab stamper | `fantasy-rogue/src/map/prefab.rs:685-990` | output via `emit`, content half stays game-side |
| Turn heap | any of the three | generic `Id`, `lwr`'s `PartialEq` |
| `ProcessingPhase`, `TurnState` | `fantasy-rogue/src/core/turns.rs:273-410` | engine-owned, no game intents registered |
| Mutation message pattern | `roguelike_engine/src/map/mutation.rs` | strip moss and fungus |
| `DamageEvent` shape, `CombatPhase` | `fantasy-rogue/src/combat/mod.rs:1195-1372` | staged pipeline replaces the monolith |
| `accumulate_equip_effects`, `merged_with` | `fantasy-rogue/src/combat/equip_stats.rs:189-226`, `items/data.rs:289-332` | generic `StatId` accumulator |
| `TimedStatus`, `DotStatus`, `expire_status::<T>` | `fantasy-rogue/src/combat/mod.rs:2216-2335` | driven by a `StatusDef` registry |
| Tactic dispatch, `TacticCtx`, pure decisions | `fantasy-rogue/src/actors/ai/{brain,decisions}.rs` | `Tactic` trait, snapshot resource instead of 16 queries |
| Ability scorer | `fantasy-rogue/src/actors/ai/targeting.rs` | score over a hook trait, not `AbilityEffect` |
| `PathCache`, `trace_shot` | `fantasy-rogue/src/actors/ai/pathfinding.rs`, `combat/archery.rs:120-147` | as-is |
| Auto-explore pure half | `fantasy-rogue/src/actors/auto_explore.rs` | `ExploreInterrupt` trait for stop rules |
| Affixes, enchant | `fantasy-rogue/src/items/{affixes,enchant}.rs` | tags instead of `ItemCategory` |
| Registry loaders and validate hook | `fantasy-rogue/src/actors/monster_data.rs:832-878` | one `ContentRegistry<T>` |
| Gas diffusion kernel | `fantasy-rogue/src/map/gas.rs:174-268` | becomes the reference `TileField<T>` kernel |
| Balance checker | `fantasy-rogue/src/actors/balance.rs` | `ThreatSubject` trait |
| `SaveBackend`, `web_unload` | `fantasy-rogue/src/save/{backend,web_unload}.rs` | as-is, plus `SaveId` remap |
| `Renderable`, screenshot | `lwr/src/{render,screenshot}.rs` | engine-owned `Cell` |
| Render rules (lit vs remembered, z-priority, lazy spawn) | `fantasy-rogue/src/render/mod.rs` | behind `GlyphGrid` |
| Narration seam | `fantasy-rogue/src/combat/narrate.rs` | `Narrator` trait |
| Theme tokens, list-detail, key hints, tabs, modals, log | `fantasy-rogue/src/ui/{theme,list_detail,key_hint,tabs,game_log}.rs` | modal registry, widget must not import its consumer |
| Test fixtures | `lwr-world/src/local/fixtures.rs` | public `rl-test-support` |
| Criterion harness | `roguelike_engine/benches/` | replace every case |

Not ported: `lwr`'s `Scale` / `Travel` mode switch, GOAP, `roguelike_engine`'s `squad/` and `stealth/noise.rs`, `lwr`'s two-entities-per-cell terminal, bracket-lib, petgraph.

## 6. What is not carried

- `#[non_exhaustive]` + `Custom { id }`, and every `_ =>` arm the compiler calls dead.
- Closed taxonomy enums in engine types.
- bracket-lib and petgraph.
- Hand-rolled generators.
- One text entity per tile.
- `HashSet<Point>` viewsheds and any hash container in a gameplay path.
- Occupancy inside the terrain resource.
- `String` def ids in hot loops.
- Melee inside the movement handler.
- Engine plugins registering other modules' messages.
- Ungated `Update` systems in engine plugins.
- `std::time::Instant` in anything that must run on wasm.
- Unseeded constructors and fresh generators inside per-turn systems.
- `TODO` comments in source.
- `lwr`'s "no world may move" refactoring constraint.
  It is right for a live game and pure cost for a library nobody has generated worlds with.

## 7. Open questions

Parked by Nate's decision to build first and course-correct.
Defaults taken: region size 64x64; lakes only where hydrology makes them.

## 8. Milestones

Each milestone is a playable slice of WildReach, and the engine ships only what that slice pulls on.
Each ends with a runnable example in `rl-engine` and a tagged build of WildReach.

**M0. Scaffold.**
Two repos: `rl-engine` and `wildreach`.
Workspace, tier layout, CI (tier-1 `cargo tree` check, `deny(missing_docs)`, doc-tests, clippy without the blanket allows, criterion baseline, wasm build check), profile trick, and a `CONTRIBUTING` page carrying sections 3.11 to 3.13.

**M1. Walk the world.**
Crates: `rl-core`, `rl-grid`, `rl-mapgen` (chain only), `rl-world` (graph, hydrology, chunks), `rl-test-support`, `rl-bevy`, `rl-render`, `rl-overworld`, `rl-ui` (shell), `rl-engine`.
Engine: grid, geometry, `RunSeed`, tile registry, bitset FOV, A*, `DijkstraMap`, `SpatialGrid`, `WorldGraph` from noise with hydrology, chunk generation with seamless boundaries, chunk streaming as the active region, the engine-owned turn loop, viewport glyph grid, the `WorldGraph` view, the overworld screen with a portal picker, theme, side panel, log, key hints.
Game: a seeded 4096x4096 world, an `@` that walks it with FOV across chunk boundaries with no visible seam, a look command, an overworld screen showing biomes, temperature, rivers and the player's marker.
The "same seed twice is byte-equal" test per chunk, a "neighbouring chunks agree at every boundary tile" property test, "every river reaches sea or lake", and benches for FOV, A*, `DijkstraMap`, one chunk, and the world graph.
Example: `walk` and a headless `world_dump --seed N --region X,Y`.

**M2. Things that hunt.**
Crates: `rl-content`, `rl-rules` (combat), `rl-ai`.
Engine: registries with validate-on-load, stats and modifiers, staged damage pipeline, `AttackIntent`, statuses, factions matrix, tactic-priority AI over `DijkstraMap` per `MovementProfile`, flee maps, stealth and awareness, semantic log with a `Narrator`.
Game: wilderness encounters from RON, death, a victory-less loop.
Example: `hunt`.

**M3. Loot.**
Crates: `rl-rules` (equipment), `rl-ui` (inventory widgets).
Engine: slot graph, item tags, affixes and enchant, pickup / equip / drop / use handlers, throwing, `BandedWeightedTable`, list-detail and tabbed inventory, character sheet.
Game: loot drops, equipment, consumables.
Example: `loot`.

**M4. Places.**
Crates: `rl-mapgen` (dungeon builders), `rl-world` (settlements).
Engine: room accretion, BSP, caves, cullers, doors, stairs, choke map, decoration rules, prefab stamping, settlement and vegetation passes inside a chunk, roads and river channels crossing chunks as `LinearFeature`s, bridges and fords, map stack with `MapBound`, transitions, discovery, town portals.
Game: towns on the map, roads and rivers between them, dungeon entrances, descend and return by portal, portal to a discovered town from the overworld screen.
Example: `places`.

**M5. Encounters and quests.**
Crates: `rl-events`.
Engine: typed world events for every gameplay outcome, trigger registry, named counters, abilities and targeting, `TileField<T>` for fire and gas, lighting.
Game: scripted and generated encounters, a quest log, a generated victory condition as data over events.
Example: `quest`.

**M6. Persist and tune.**
Crates: `rl-save`, `rl-tools`.
Engine: save backend with `SaveId` remap and versioning policy, chunk delta persistence, balance checker over `ThreatSubject`, seed replay.
Game: save and continue with mutated chunks restored, a balance report over its RON.
Example: `save_roundtrip`, `balance_report`.

**M7. Corsair.**
A small pirate example game as a workspace member of `rl-engine`, a few thousand lines with its own RON.
It is chosen because it stresses different seams from WildReach.

- A world that is mostly ocean: the quantile banding knob turned the other way, islands as regions, ports as sites.
- Water walkable only by ships: `MovementProfile` per class, so navy and merchant ships and land walkers each get their own `DijkstraMap`.
- The player's ship as a vehicle: boarding switches the player's profile, no sub-map.
- Boarding combat: an adjacent ship pushes a small deck map onto the map stack.
- Three factions with a relation matrix: navy, pirates, merchants.
- Non-fantasy vocabulary registered from RON: cannon and cutlass damage, scurvy as a status, rum as a consumable, doubloons as loot.
- A treasure map quest as data over events: dig at a marked tile.
- Weather as a `TileField<T>`.

The point is not the game; it is that nothing in tiers 0 to 2 changes to make it possible.
If something does have to change, that is a seam the plan got wrong.
