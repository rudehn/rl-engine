# rl-engine: duplication between the engine and its games, and how the crates and plugins are cut

Written 2026-10-12, against `main` at `77e2904`.
Scope: every module of `examples/undermere/src`, compared with the engine crates and with Foundry, Corsair, Delve, Heist and the tutorial; then the crate split and the plugin model.
Method: three read-only sweeps over Undermere's world and generation, its items, combat and monsters, and its mercenaries, app and glue, each counterpart read rather than matched by name; the bugs below were confirmed by reading the code again.
Nothing was changed.

Later the same day the user removed Corsair, Delve and Heist from the workspace, keeping the tutorial, Undermere and Foundry.
The game counts below are as found and still show how often a game had to write the thing itself; each finding now applies to Undermere and Foundry, and to the tutorial where it is named.

The project's two tests for "this belongs in the engine" are the ones applied throughout.
One is "own the loop or leave it out": the engine ships behaviour, not a data structure each game re-implements the loop for.
The other is the second game: the same thing written in two games is a field or a system the engine is missing.

## The short answer

The engine is missing about a dozen loops that three to five games each wrote for themselves, and those are where the duplication is.
Stairs between floors, monster population, the player's key loop, the screen layout and panel set, the item and monster definition schemas, scripted runs that must not touch the player's save, and the game test harness are each written in three or more games, often line for line.
Undermere alone adds a second layer: loops the engine ships only the data for, such as identification (`Unknown` is a marker and nothing more), what an enchant level does, what worn gear lends, auras, and effects that make actors.
The crates are cut well and should stay as they are, with one small exception, `rl-overworld`.
The plugins should not all be independent: what matters is that every dependency is declared and checked, which it mostly is.

## 1. Bugs found on the way

- **A scripted Corsair run writes over the player's save.**
  `examples/corsair/src/main.rs:188` inserts `Saves::platform_default("corsair")` unconditionally, so a replay, a recording or a capture of Corsair saves into the player's slot.
  Undermere (`main.rs:315` `scripted`, `:325` `saves_for`) and Foundry (`main.rs:212`, `:222`) each guard against this with the same two functions and the same test, which is the case for moving it into `RoguelikePlugins` (finding 2.7).
- **`DamageDealt::dealt` is documented as what health lost, and reports the raw blow.**
  `crates/rl-bevy/src/combat.rs:542` says "What health lost"; `:1309` reports `amount` unclamped, so a killing blow past the target's last health reports more than it lost.
  Undermere's `tally.rs:211` reconstructs the clamped figure by hand.
  Either clamp it, or keep the raw blow and add a `lost` field, and say which in the doc.
- **Undermere's waystone kneeling no longer behaves as its doc says.**
  `examples/undermere/src/waystone.rs:155` `kneel` claims the player's turn in `DecideSet::Game`, so the engine's `work_for_player` (`crates/rl-bevy/src/work.rs`, since `37d177a`) always skips it: kneeling has no `WorkPace` and is not broken off by a foe coming into sight.
  Its doc's "whatever key is pressed" is already untrue in the real game, since `rl-ui`'s `stop_work_on_key` leaves the work off on any key.
  Deleting `kneel` and its registration hands kneeling to the engine; `captives.rs`'s one-action freeing (`FREE_TIME`, written because "the engine's multi-turn work skips the player") can become interruptible work the same way.
  Both are polish wave B1's first task.

## 2. Written in three or more games

Each of these is the strongest kind of finding: the same code in several games, and the engine with nothing.
Sizes are the engine change; each also deletes code from every game listed.

| # | What | Where it is written | Engine today | Move to | Size |
|---|---|---|---|---|---|
| 2.1 | Stairs laid on first arrival, and the stair marker saved | undermere `stairs.rs:124`, foundry `lifts.rs:55`, delve `main.rs:424`, heist `main.rs:509`, corsair `places.rs:185`, tutorial steps 6 to 10; saved by `undermere/save.rs:438`, `foundry/save.rs:207`, `corsair/save.rs:171` | `Transition` exists (`places.rs:88`); nothing lays it | `rl-bevy` places: a plugin taking glyphs, names and a last-floor hook | medium |
| 2.2 | Monster population on first arrival | undermere `population.rs:145`, foundry `droids/spawns.rs:40` (near line for line), delve `main.rs:499`, heist `main.rs:588`, corsair `places.rs:231` | loot has the exact shape (`rl-rules/loot.rs:518` `plan_scatter`, `rl-bevy/loot.rs:242` `scatter_places::<M>`); monsters have none | `rl-rules` planner and `rl-bevy` `populate_places::<M: ActorMaker>` | medium |
| 2.3 | The player's keys to intents: walk, go through, wait, pick up, fire, throw from the pack | undermere `input.rs:40,97`, foundry `input.rs:41,110` (near line for line), corsair `input.rs`, delve `main.rs:548`, heist `main.rs:752` | `add_control` and `ControlInput` only | `rl-ui`: a standard player-keys plugin a game adds its own verbs to | medium |
| 2.4 | The screen cut and the panel set: rail, log, vitals, gear, nearby, hint, target over nearby | undermere `main.rs:107,201`, foundry `main.rs:77,136`, corsair `main.rs:80`, delve `main.rs:121`, heist `main.rs:120` | every panel, no layout | `rl-ui`: a rail layout builder and a default panel set a game edits | medium |
| 2.5 | Item definitions and spawning from them | undermere `armory.rs:179,880` (its doc: "Foundry's item schema trimmed"), foundry `gear.rs:57,572` with `content.rs:70,92`, corsair `items.rs:36,274` | `ItemMaker` without a schema behind it | `rl-bevy` items: a core def and spawn-from-def as a default `ItemMaker`, games adding fields | large |
| 2.6 | Monster definitions, spawning and the brain built from them, the same tactic chain in all three | undermere `bestiary.rs:54,564,756`, foundry `droids.rs:48,235,294`, corsair `monsters.rs:30,128` | `ActorMaker` without a schema | `rl-bevy` def and spawn, `rl-rules` ai brain-from-def | large |
| 2.7 | A scripted run saves in memory | undermere `main.rs:315,325`, foundry `main.rs:212,222`, the same test in both; corsair missing it (the bug above) | `RoguelikePlugins` already reads `capture::requested()` | `rl-engine`: `RoguelikePlugins::saves(name)` picks memory or the platform | small |
| 2.8 | The game test harness: `headless`, `settle`, `pass_turns`, `continued`, `row`, `written` | undermere `testing/mod.rs:36-53`, foundry `testing/mod.rs:54-198`, screen readers in both `main.rs` | `rl-bevy::testing`, `rl-ui` `Stage::row` on a `Stage` only | `rl-bevy::testing` and `rl-ui` harness, as functions on an `App` | small to medium |
| 2.9 | The quest file loader, and a victory quest ending the run | undermere `quest.rs:42-98,153`, foundry `mission.rs:37-92,238`, corsair `quests.rs:59-149,246` | facts and quests tracked, fed in `PostUpdate`, too late for a win in the same pass | `rl-rules` loader generic over the game's `On`; `rl-bevy` ends the run on a victory quest, fed in `TurnSet::React` | small to medium |
| 2.10 | Ambient light per place, written every frame | undermere `light.rs:191`, foundry `light.rs:51`, delve `main.rs:394` | `Lighting::ambient` | `rl-bevy` lighting: an ambient on `PlaceBuild` applied by the engine | small |
| 2.11 | The enchant level saved by hand | undermere `save.rs:363`, foundry `save.rs:165`, corsair `save.rs:101` | `Enchant` is an engine component that `EntityState` does not capture | with 2.5, not `rl-save` alone: each game saves the level inside its item record because it rebuilds the item from its def at that level, so the engine saving `Enchant` beside it would be redundant until the engine owns the def (decided 2026-10-12) | small, inside 2.5 |
| 2.12 | Effects reacting to damage dealt, read off `DamageDealt` by hand because `hit` and `struck` fire before damage is known | undermere `tricks/bodies.rs:33,95`, corsair `statuses.rs:31`, foundry `droids/sensors.rs` | moments before damage only (`effects/triggers.rs:57-68`) | `rl-bevy` effects: a `wounded` moment after damage, carrying the amount, and `Split` and `Drain` effects | medium |
| 2.13 | Lit tiles and lit props as fixtures | undermere `light.rs:219-294` with its own `LightDef`, foundry `light.rs:74`, delve `main.rs:459` | no light on a tile or prop def; only fire calls `set_glow` | a `light` field on `rl-rules` `TileDef` and `PropDef`, lit by `rl-bevy` on arrival and restore | medium |
| 2.14 | Gear score, armor twice plus the average blow plus half the average shot | undermere `armory.rs:785`, corsair `items.rs:266` | `GearScore` leaves the scale to the game | a default scorer in `rl-bevy` items | tiny |

## 3. Written in two games

- **A key spent when it opens a lock**: undermere `vaults.rs:393`, foundry `props.rs:225`; `doors.rs:115` spends the lock and never the key, and `DoorEvent::Opened` does not say a lock was opened, so Undermere reads it off the tile. A consume-key option on `Lock`, with the key in the event. Small.
- **A single-use prop turned into its spent kind**: undermere `vaults/wells.rs:151` and `vaults/lifts.rs:75`, foundry `mission.rs:197`. A `spent` field on `PropDef`. Small to medium.
- **The deepest floor reached**: undermere `tally.rs:161`, foundry `climb.rs:32`, the same `PlaceEntered` max. Saved, in `rl-bevy` places. Small.
- **Dark sight lent by worn gear**: undermere `wearing.rs:74-139`, foundry `gear.rs:264` with `droids/sensors.rs:120`, both taking the best of the wearer's own and the worn. Folded into finding 4.3.
- **What an enchant level does**: undermere `armory.rs:418-1037`, foundry `gear.rs:195`; the engine's `EnhanceRule` (`rl-rules/affix.rs:210`) covers stats and damage per level and only Corsair uses it. Finding 4.2.
- **The builder hidden behind `PlaceRulesRes`**: `Box<dyn PlaceRules>` gives no way back to the game's builder, so undermere (`light.rs:105`, `BiomeDraws`, `VaultTiles`, `Fixtures`) and foundry (`light.rs:46`, whose doc names the reason) copy its data into resources. An `as_any` on `PlaceRules`. Small.
- **The save restore helper** `spawned`: byte-identical in undermere `save.rs:97` and foundry `save.rs:62`, inlined in corsair `save.rs:107`. `rl-save`. Tiny.
- **The title screen's `hashed`**: byte-identical in undermere `title.rs:203` and foundry `title.rs:173`, and a copy of `rl-core`'s `position_hash`. Tiny.
- **A schedule ambiguity check**: foundry `plugin/ambiguity.rs:23,38` walks every schedule for unordered conflicting pairs, and nothing in its walk is Foundry's; Undermere has no such guard. The generic half to `rl-bevy::testing`. Small.
- **"Start on place N" for a photograph**: `UNDERMERE_START`, `FOUNDRY_START`, `CORSAIR_START`. Beside `rl-render`'s capture switches. Small, borderline.

## 4. The engine ships the data, Undermere wrote the loop

One game each, but each is the engine's own half-built subsystem, which is the "own the loop" rule rather than the second-game one.

- **4.1 Identification.** The engine has `Unknown` (`rl-bevy/items.rs:180`) and nothing else. Undermere built the rest across `lore.rs` (per-run appearances from the seed, learning by use and by asking), `seeming.rs`, `armory.rs:394,402`, `shops.rs:443` (learning by buying) and `effects.rs:58` (a use that showed nothing teaches nothing). On the way, `seeming.rs:159,188` copies `Marksmanship::odds` and `at_distance` (`accuracy.rs:88,114`) line for line only to subtract some accuracy, and `seeming.rs:217` rebuilds the inspect collector's forecast. An accuracy offset on `odds` and an "unknown gear counts at plain" adjustment for the collectors are small and remove the copies; the loop itself is large.
- **4.2 What an enchant level does, and the curse loop.** `EnhanceRule` extended to armor, effect bonus, charges, pulse period and dark sight; an engine `level_up` that rebuilds from the def; then `Enchanting` and `RemoveCurse` (`enchant.rs:33,92`), since the engine's `Stuck` (`items.rs:191`) says "what frees it is the game's". Medium.
- **4.3 What worn gear lends.** The engine ends a worn thing's statuses (`items.rs:834`) and nothing starts them; Undermere's `wearing.rs:94` re-applies them, lends dark sight and lights a worn light. `WornDarkSight`, `WornStatuses` and worn-only light in `rl-bevy` items. Small to medium.
- **4.4 Status behaviours keyed by status name.** Confusion stumbles (`conditions.rs:43`), discord swaps the faction (`:89`), fireproof lends a resistance (`:125`), a stack capped at three (`tricks/bodies.rs:174`), a modifier while another status is held (`tricks/auras.rs:38`). `StatusDef` already carries `holds`, `sleeps` and `unseen`; `confuses`, `faction`, `resists` and `Stacking::Stack { max }` join them. Medium.
- **4.5 Auras.** `tricks/auras.rs:91` and `artifacts.rs:42`: a status or a modifier on same-side actors in reach, taken back when they leave. An `Aura` component in `rl-bevy`. Small to medium.
- **4.6 Effects that make actors.** `effects/bodies.rs` `Summon`, `Polymorph`, `Plenty`, `Conjure` (fades after N turns), and `tricks/bosses.rs:102` "dies with its maker", all expressible over `ActorMaker`. Medium.
- **4.7 Movement effects.** `Beckon` (`effects/moving.rs:179`) is the engine's `Pull` with the distance as its cells; `Blink` stops at the last free cell where the engine's `Teleport` refuses; random far teleport for self and other. Small.
- **4.8 Stats bridged to engine components.** `stats.rs` keeps stealth, footfall, keen and regeneration from stats every pass; `CombatRules` and `TurnRules` already name their stats, and noise, stealth and regeneration have no such ids. Small each.
- **4.9 The party.** Followers come through a way or a warp within reach and sight (`mercenaries/travel.rs:54`), where the engine's own follow-through (`places.rs:343`) covers only `TRAVELS` minds one step away through `GoThrough`, so `orders.rs:199` strips `TRAVELS` to stop it; `WarpRequest` has no `after`, so the narrator says "leaves" for a follower and `say_who_came` patches it; the rail lists a follower twice unless the game removes it from nearby (`orders.rs:309`); a follower's death is told in the good tone (`narrate.rs:1114`, patched by `leaving.rs:157`); a mind cannot equip from its own pack (`Decision` has `EquipFromGround` only, worked round by `knowing.rs:172-205`); the leash wrappers `Leashed` and `Behind` (`orders.rs:84,117`) wrap the engine's `Led` and nothing else. Each is small; together they make a party subsystem the next game with followers gets for free.
- **4.10 The fight forecast for someone not hostile.** `rl-ui/view/inspect.rs:266-300` forecasts a duel whatever the relation, so the tavern, the orders and the townsfolk each clear it (`tavern.rs:331`, `orders.rs:286`, `townsfolk.rs:672`). Forecast only for the hostile. Small.
- **4.11 Leaving play without dying, and a summon's lifetime.** `allies.rs:73` `vanish` is `process_deaths`' teardown without a `DeathEvent`, used three times, and drops the inventory; `Fades` is a lifetime with a line. Small.
- **4.12 Generation passes.** `dungeon.rs` `Patches` (blob growth), `Furnish` with a floor-wide no-cut guard that `Dress` has only per area, `Underfoot`, and `Paint` (a floor tile by area shape and district, names the engine records and gives no meaning). `rl-mapgen`. Medium.
- **4.13 Kind-first floor loot.** `finds.rs:97,163` replaces `scatter_places` because it draws flat from one table: tag-weighted kinds and always-laid items on `ScatterRules`. Medium; touches the play notes' task D6, so after it.
- **4.14 Hidden traps.** `traps.rs:304` `LayHidden` on corridors, chokepoints and doorways from the `Survey` sits naturally beside `rl-mapgen`'s `HideOpenings`; the narrator has no `Spotted` phrase (`traps.rs:529`). Borderline.

## 5. Small helpers to expose or add

- A weighted pick in `rl-core`: Undermere rolls it twice (`biomes.rs:386`, `finds.rs:105`) and the engine itself six times (`loot.rs:183`, `namegen.rs:209`, `pool.rs:645`, `content/table.rs:123`, `affix.rs:342`, `mapgen/prefab.rs:622`).
- `LootTable::pick_distinct(n)`, which `shops.rs:240` builds for its shelves.
- BFS: `dungeon.rs:786` and `vaults.rs:230` duplicate `rl-grid`'s `region::flood` and `DijkstraMap::build`.
- `CapArmor { pct }` beside `SubtractArmor` in `rl-rules/damage.rs`: `damage.rs:49` `HalveArmor` has nothing of Undermere in it.
- A public `Called::of(entity)` in `rl-ui`: the name with its stack and `MassNoun` counted is written three times in the mercenaries (`leaving.rs:41`, `knowing.rs:318`, `tavern.rs:277`) beside the narrator's private `called`.
- `trade.rs:313` `hand_over` made public: `knowing.rs:310` copies it without the merge into a matching stack, so a mercenary given potions twice holds two stacks.
- Status names on the nearby rail: `conditions.rs:167` adds `StatusDef::badge` and "asleep" to nearby rows, both engine fields the vitals panel already shows for the player.
- Small effects: heal to full, cure every lasting status, recharge by tag, magic mapping (`effects/mending.rs`, `charges.rs`, `senses.rs`).
- Game modifier sources are packed into a `u64` by hand (`artifacts.rs:29`), so two of a game's sources can collide; intern them by name in `rl-rules`.
- An offer's label template declared with its verb: `tavern.rs:361`, `shops.rs:487` and `temple.rs:186` each rewrite "{verb} {what}".

## 6. Judged the game's own

The rest is Undermere's content, rules and wording, and is right where it is: the biomes, lairs and pool pieces; vault marks, causeways, levers, lifts and wells; the town stamp and Mereholt's gate; camps, feeders, patrols and the captive draw; bosses' clocks; rumours, writings, the temple's rites, the tally's score lines; the mercenaries' fees, kits, knowledge rules, dismissal and fallen; the title art; content manifests and debug starts.
Promoting any of them would put Undermere's rules in every game.

## 7. The crates

**What a crate means here.**
A crate is Rust's unit of compilation and of dependency: the compiler enforces its boundary, builds it as a whole, and a consumer opts into it in `Cargo.toml`.
In this workspace a crate means a dependency boundary, and only that: its tier (whether it may see Bevy) and its layer (rules, simulation, drawing, interface, saving).
A subsystem is a module and a plugin inside its layer's crate, never a crate of its own.
That is the right meaning, and it is the one `check-tiers.sh` enforces.

**What the code says about splitting further.**
`rl-bevy` is 36,000 lines in 47 files with 24 plugins, and `rl-ui` 24,000 in 48 with 37.
Leaving tests out, the centre of `rl-bevy` imports itself in a ring: `turn` uses `combat`, `combat` uses `minds`, `items`, `ability`, `status` and `stealth`, and each of those uses `combat` back, with `places` and `props` in the same knot.
That is one simulation, and a crate border cannot follow a cycle: splitting it would mean inventing traits and messages for the border's sake, not the design's.
The edge of `rl-bevy`, `fire`, `gas`, `fields`, `ammo`, `loot`, `remains`, `records`, `charts`, `title`, `settings` and `work`, imports the centre and is imported by almost nothing, so it could move out; the gain is build time and nothing else.

**Build time, measured.**
With a load average near 180 from another session's five builds, so the figures are relative: touching `rl-bevy/src/fire.rs` cost 34s to rebuild Undermere, against 12s for an `rl-ui` panel and 13s for an Undermere module.
Every `rl-bevy` change rebuilds `rl-render`, `rl-ui`, `rl-save`, `rl-overworld`, the facade and the game above it.
An edge crate would skip that for edge work only, and most engine work lands in the centre.

**Recommendations.**
- Keep the tiers and the layers; keep subsystems as plugins.
- Do not split `rl-bevy` by subsystem. Revisit only if a measured edit-build loop on an idle machine becomes the bottleneck, and then move the edge modules out, not the centre.
- Fold `rl-overworld` into `rl-ui`. It is 346 lines and one plugin, a screen like every other screen in `rl-ui`, and the only subsystem that is a crate; keeping it one breaks the rule the rest follow.

## 8. The plugins

**What there is.**
Sixty-six plugins across `rl-bevy`, `rl-ui`, `rl-render`, `rl-save` and `rl-overworld`.
About sixty hard dependencies are declared with `depends_on`, thirty-four requirements with `app.needs::<R>`, and every missing one is reported together when play begins.
Soft links go one way, the higher plugin noticing the lower with `is_plugin_added`: each panel adds its view plugin, and `UiPlugin` adds the stop key when `WorkPlugin` is there.
`RoguelikePlugins` adds the base in one line.

**Should they all be independent?**
No.
Independence is the wrong goal for a simulation whose centre is one knot: combat without turns, or stealth without minds, is not a smaller engine but a broken one.
The right properties are the ones the engine mostly has: every plugin opt-in, every dependency declared and checked loudly at startup, soft integrations in one direction only, and no plugin deciding whether it runs by whether a resource happens to exist.

**Where it falls short.**
- The findings above show the real gap is not coupling but missing plugins: a game assembles stairs, population, keys, layout, panels, defs and harness by hand, so a new game starts from a copy of an old one. A second group beside `RoguelikePlugins`, a standard play screen of keys, rail layout and default panels, is what a new game would add after 2.3 and 2.4.
- Two places where a game has to switch engine behaviour off by editing data rather than by choosing: `orders.rs:199` strips `Wits::TRAVELS` to stop the engine's follow-through, and Undermere's `MereholtPeace` (`damage.rs:71-108`) needs an `Arc<AtomicBool>` because a `DamageStage` cannot see which map is loaded. Each wants an option on the plugin or the place, not a workaround.
- There are 123 `Option<Res<_>>` parameters in engine systems. Most are optional data, which the rules allow, but this review did not audit them for the one thing the rules forbid, a subsystem switched on by a resource's presence.
- The soft links are written down only in the code. A table in the overview of which plugin lights up what, when added beside which, would make them discoverable.

## Suggested order

1. The three bugs in section 1: small, and the waystone and captives are polish wave B1 already.
2. The cheap three-game items: 2.7 scripted saves, 2.10 ambient light, 2.14 gear score, 2.9 quests, 4.10 the forecast with a peaceful place, and the section 5 helpers; 2.11 the enchant level goes with 2.5.
3. The big three-game items, each its own slice with its own design note: 2.1 stairs and 2.2 population first (five games each), then 2.3 keys with 2.4 layout as the play screen group, then 2.5 and 2.6 the def schemas, which are the largest and touch every game's content.
4. The Undermere-only loops by how many future games would want them: 4.1 identification, 4.2 levels and curses, 4.3 worn gear, 4.9 the party.
5. The play notes now running (`docs/superpowers/plans/2026-10-12-play-notes.md`) own Undermere's assets, `shops.rs`, `mercenaries/kit.rs` and the nearby and party panels; 4.13, 2.4 and the party items wait for that branch to merge.
