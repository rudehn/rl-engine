# fantasy-rogue: combat / actors / items / ui / assets - engine-extraction review

Scope: `src/combat/`, `src/actors/`, `src/items/`, `src/ui/` (skim), `assets/*.ron`.
Sibling reviewer covers `core/`, `map/`, `render/`, `save/`, `audio/`.
Totals in scope: combat 20,344 LOC, actors 13,362, items 11,272, ui 17,652 ≈ 62.6k of the 92.5k tree.
Note on LOC: this repo carries roughly 1:1 test-to-production code inside `#[cfg(test)]` at the bottom of
each file. `src/combat/mod.rs` is 7,620 lines but production ends at line 3,208. Where it matters below I
quote `total / prod`.

---

## A. Subsystem inventory

### combat/

| Subsystem | Path | LOC (tot/prod) | Purpose | Verdict |
|---|---|---|---|---|
| Damage math primitives | `src/combat/mod.rs:347-520` | ~180 | `attack_hits`, `compute_after_armor`, `range_to_hit_penalty`, `apply_crit`, `apply_damage_modifiers`, `apply_attack_mult` | **EXTRACT-AS-IS** (except `sneak_mult`) |
| Damage event pipeline | `src/combat/mod.rs:1565-2180` | 615 | `damage_application_system` - the single `DamageEvent` consumer | **EXTRACT-WITH-REDESIGN** |
| `DamageType` / `ResistLevel` / `Resistances` | `src/combat/mod.rs:52-310` | 260 | typed damage + tiered resistance ladder + accumulator | **EXTRACT-WITH-REDESIGN** (variant set is content) |
| Status effects (components + apply + tick) | `src/combat/mod.rs:576-960, 996-1143, 2216-2340, 2434-2900` | ~1100 | 20 status components, `apply_status`, per-status tick systems | **EXTRACT-WITH-REDESIGN** (the `TimedStatus`/`DotStatus` traits are the salvageable part) |
| `CombatPhase` / `CombatEventSet` | `src/combat/mod.rs:1329-1372` | 45 | Pre/Apply/Post ordering contract | **EXTRACT-AS-IS** |
| Effect vocabulary | `src/combat/effects.rs` | 528/~400 | `OnHitEffect`/`OnDefendEffect`/`OnKillEffect`/`OnDeathEffect`/`OnBlockEffect`/`DamageMod`/`StatusEffect`/`CombatEffects` | **EXTRACT-WITH-REDESIGN** - shape is engine, variants are content |
| Equip-stat accumulator | `src/combat/equip_stats.rs` | 2404/628 | `EquipAccum` + `accumulate_equip_effects` + `recalculate_stats` | **EXTRACT-WITH-REDESIGN** - the concept is a modifier stack, the impl is 25 named fantasy fields |
| Actions / intents / turn cost | `src/combat/actions.rs` | 3539/1449 | `Action` enum, intent messages, `dispatch_player_action`, `handle_movement` (which also contains melee), `SpeedStats`, `finish_turn`/`free_turn` | **EXTRACT-WITH-REDESIGN** - split attack out of movement first |
| Abilities / casting | `src/combat/abilities.rs` | 3411/1591 | `AbilityDef`, `TargetMode`, `AoeShape`, `CastIntent`, `handle_cast_intent`, footprint resolution | **EXTRACT-WITH-REDESIGN** - `AoeShape` + footprint resolution are pure engine; `AbilityEffect` is content |
| Archery / projectiles | `src/combat/archery.rs` | 672 | ammo model + `trace_shot` (shared with throwing) | `trace_shot` **EXTRACT-AS-IS**; the ammo/bow model **GAME-SIDE** |
| Death / lifecycle | `src/combat/lifecycle.rs` | 762 | `process_deaths`, GameOver transition, corpse decay | **EXTRACT-WITH-REDESIGN** |
| Narration | `src/combat/narrate.rs` | 371/197 | `HitNarration` → English log sentence | **GAME-SIDE** (but the *seam* is an engine idea - see F7) |
| Status badges | `src/combat/status_view.rs` | 397 | one `StatusQuery` → ordered glyph/colour/turns badges for 3 surfaces | **GAME-SIDE**, pattern worth copying |
| Phylactery | `src/combat/phylactery.rs` | 640 | Lich die-and-reform, paired by `pair_id` | **GAME-SIDE** |

### actors/

| Subsystem | Path | LOC | Purpose | Verdict |
|---|---|---|---|---|
| AI brain | `src/actors/ai/brain.rs` | 2756/1642 | per-turn tactic dispatch, `TacticId`, `TacticCtx`, wander/patrol | **EXTRACT-WITH-REDESIGN** - excellent architecture, closed enum + Bevy-param-ceiling problems |
| AI pure decisions | `src/actors/ai/decisions.rs` | 499 | `should_flee`, `should_kite_retreat`, `notices`, `flee_direction`, `threat_priority`, `advance_waypoint` | **EXTRACT-AS-IS** |
| Pathfinding | `src/actors/ai/pathfinding.rs` | 573 | A* wrappers, `HazardMap` cost overlay, `PathCache` | **EXTRACT-AS-IS** (the `HazardMap` overlay pattern especially) |
| AI ability scoring | `src/actors/ai/targeting.rs` | 1074 | `best_cast` over `CasterView`/`EntityView`, AOE-summed utility incl. friendly fire | **EXTRACT-WITH-REDESIGN** - genuinely good, pure, but scores `AbilityEffect` variants directly |
| AI state component | `src/actors/ai/monster_ai.rs` | 237 | `MonsterAIMode` + knobs + tracking state | **EXTRACT-AS-IS** |
| Gear evaluation | `src/actors/ai/gear.rs` | 369 | precomputes "is that sword better than my claws" into `GearOptions` | **GAME-SIDE**; the precompute-to-dodge-param-ceiling trick is a workaround, not a design |
| Monster data | `src/actors/monster_data.rs` | 2071/1040 | `MonsterDef`, `AiDef`, `LootEntry`, `SpawnEntry`, registries, RON seams | **EXTRACT-WITH-REDESIGN** → generic `ContentRegistry<T>` + `BandedSpawnTable` |
| Spawner | `src/actors/spawner.rs` | 3251/1821 | monster + player construction, Voronoi placement, summons, cache restore | **EXTRACT-WITH-REDESIGN** - placement is generic, construction is content |
| Leveling | `src/actors/leveling.rs` | 1199 | XP curve, per-level HP/to-hit/dodge grants, summon kill credit | **EXTRACT-WITH-REDESIGN** - the curve/award seam is generic, the grants are balance |
| Balance checker | `src/actors/balance.rs` | 2140/1416 | offline `Threat = EffHP × DPS` scorer + HD scoping + spawn progression | **EXTRACT-WITH-REDESIGN** - best analysis tool in any of the three repos; needs a trait |
| Auto-explore | `src/actors/auto_explore.rs` | 1478 | frontier Dijkstra over `explored_tiles` via `KnownMap`, greedy pickup, interrupt rules | **EXTRACT-WITH-REDESIGN** - the algorithm is engine, the interrupt list is game |
| Horde spawner | `src/actors/horde.rs` | 216 | periodic off-screen reinforcement with population cap | **EXTRACT-AS-IS** (tiny, well-seeded) |
| Character stats aggregation | `src/actors/character_stats.rs` | 1198 | pure snapshot helpers for the Character/Inventory screens | **GAME-SIDE** |
| Mutations | `src/actors/mutations.rs` | 78 | one-variant leveled trait scaffold | **DROP** (a stub; fold into the effect vocabulary) |
| Run stats | `src/actors/run_stats.rs` | 166 | per-run kill/turn/depth counters | **EXTRACT-AS-IS** (trivially generic) |
| Player input | `src/actors/player.rs` | 416 | marker + keyboard → `PendingPlayerAction` + `WallBumpGate` | **GAME-SIDE** |

### items/

| Subsystem | Path | LOC | Purpose | Verdict |
|---|---|---|---|---|
| Item data model | `src/items/data.rs` | 2151/1040 | `ItemDef`, `ItemKindData`, `ItemCategory`, `EquipEffect`, `ArmorSlot`, `BlockProfile`, `ChargeProfile`, registries | **EXTRACT-WITH-REDESIGN** - hard-bound to fantasy categories |
| Item handlers | `src/items/handlers.rs` | 4502/1326 | pickup / equip / drop / use / throw / steal / enchant / recharge intents | **EXTRACT-WITH-REDESIGN** - pickup/equip/drop/stacking is a generic equipment framework; use/consume is content |
| Affixes | `src/items/affixes.rs` | 1093/561 | `AffixDef`, `AppliesTo` eligibility, `ScaledOnHit`/`ScaledEquipEffect`, `sync_combat_effects` | **EXTRACT-WITH-REDESIGN** - best modifier-rolling model in the tree |
| Enchantment | `src/items/enchant.rs` | 878 | `+N` level, data-driven `EnhanceRule`, one-source-of-truth folds | **EXTRACT-WITH-REDESIGN** |
| Item spawn/placement | `src/items/spawn.rs` | 919 | render colour, entity construction, Voronoi floor placement, transition cache | **EXTRACT-WITH-REDESIGN** |
| Item class weights | `src/items/item_class.rs` | 226 | class-first spawn roll, data-driven weights | **EXTRACT-WITH-REDESIGN** |
| Loot pools | `src/items/loot_pools.rs` | 196 | named weighted draws | **EXTRACT-AS-IS** - deliberately flat and small |
| Throwing | `src/items/throw.rs` | 450 | solid-vs-flask split, reuses `trace_shot` and the ordinary hit contest | **EXTRACT-WITH-REDESIGN** |
| Descriptions | `src/items/describe.rs` | 566 | `EquipEffect` → English prose | **GAME-SIDE** |
| Item fire destruction | `src/items/fire.rs` | 129 | flammable item on a burning tile | **GAME-SIDE** |
| Item stat display | `src/items/stats.rs` | 143 | fold helpers for the enchant preview | **GAME-SIDE**, mostly vestigial |

### ui/ (skim)

| Subsystem | Path | LOC | Verdict |
|---|---|---|---|
| Design tokens | `src/ui/theme.rs` | 262 | **EXTRACT-AS-IS** - spacing scale, z-ladder, semantic palette, single-font installer |
| List→detail widget | `src/ui/list_detail.rs` | 758 | **EXTRACT-WITH-REDESIGN** - genuinely reusable, but imports nav math from `inventory_preview` |
| Key hints / keybinds | `src/ui/key_hint.rs`, `keybinds.rs` | ~450 | **EXTRACT-AS-IS** |
| Tabbed window chrome | `src/ui/tabs.rs` | ~350 | **EXTRACT-WITH-REDESIGN** - depends on the closed `ActiveModal` enum |
| Game log | `src/ui/game_log.rs` | 851 | **EXTRACT-WITH-REDESIGN** - ring buffer + rendering is generic; the colouring is substring-matching English (see B5) |
| Side panel / examine / targeting / inventory / character / spellbook / main menu | 8,000+ | **GAME-SIDE** |

### assets/

3,929 lines of RON across 13 files. All carry a top-of-file option-space header comment (a repo convention).
`assets/monsters.ron:1-60` and `assets/affixes.ron:1-45` are the strongest examples: they document the *full*
option space including cross-cutting rules (the automatic thermal on-hit rider, the `chance` semantics).
**GAME-SIDE**, but the header convention should be an engine documentation rule.

---

## B. Coupling & boundary analysis

### B1. Purity map

**Pure (no Bevy World, no ECS) - directly liftable:**
- `src/combat/mod.rs:347-520` damage arithmetic; `:964-994` `scale_thermal_status`
- `src/combat/narrate.rs` entirely (takes `HitView`/`HitNarration`, returns `String`)
- `src/actors/ai/decisions.rs` entirely - explicitly documented as "no ECS, no Bevy, no RNG"
- `src/actors/ai/targeting.rs` - pure over `CasterView`/`EntityView` snapshots
- `src/actors/ai/pathfinding.rs:37-200` - `next_step_toward`, `find_path`, `find_path_hazard`, `HazardMap`
- `src/actors/balance.rs` entirely - "no Bevy systems, no plugin, no RNG" (`src/actors/balance.rs:220-226`)
- `src/actors/character_stats.rs` entirely
- All `load_*_from_str` seams (11 of them, listed in C4)
- `src/combat/equip_stats.rs:189-226` `accumulate_equip_effects`

**Bevy-plugin-shaped:** `CombatPlugin`, `AbilityPlugin`, `AiPlugin`, `ItemPlugin`, `MonsterDataPlugin`,
`LevelingPlugin`, `PhylacteryPlugin`, `TabsPlugin`.

**Mixed (the problem cases):**
- `src/combat/mod.rs` - 3,200 production lines mixing pure arithmetic, ~20 component definitions, the
  registry-free plugin, and the 615-line `damage_application_system`. This one file would need splitting into
  four crates' worth of concerns.
- `src/combat/actions.rs` - `handle_movement` (`:777-1403`, **626 lines**) is a movement handler that also
  contains the entire melee attack resolution: hit contest, sneak detection, crit, off-hand proc, retaliate,
  weapon on-hit riders, door-bumping, and same-faction position swaps. There is **no `AttackIntent` in the
  codebase** (`grep -rn 'AttackIntent' src/` → 0 hits). Attacking is only reachable by walking into someone.

### B2. Dependency direction problems

`CombatPlugin::build` (`src/combat/mod.rs:1376-1420`) registers messages owned by five other modules:

```rust
.add_message::<crate::spawner::SplitMessage>()
.add_message::<abilities::BlinkMessage>()
.add_message::<crate::map::gas::GasDepositEvent>()
.add_message::<SfxEvent>()
.add_message::<StealItemMessage>()
```

The stated reason (comments at `:1402-1418`) is test self-sufficiency - `add_message` is idempotent, so each
plugin re-registers. It works, but it means "combat" transitively depends on the spawner, the gas field, the
audio layer and the item layer just to boot. `AiPlugin` does the same thing at `src/actors/ai/brain.rs:296-302`
for four item intents.

For an engine this is inverted: the message *type* should live with the consumer, and the producer should
depend on an abstract writer. Concretely: `SplitMessage`, `BlinkMessage`, `StealItemMessage` are all
"deferred reaction" messages the damage pipeline emits because it cannot mutate the relevant components
while holding `&mut Health`. That is a generic pattern (a **reaction queue**) that deserves one engine type,
not five bespoke messages each owned by a different plugin.

Other direction problems:
- `src/combat/effects.rs:138` - `OnDeathEffect::GasCloud { kind: crate::map::gas::GasKind }`. The shared
  combat vocabulary imports a map-layer content enum.
- `src/combat/effects.rs:393` - `PropEffect` (a map-props vocabulary) lives in the combat module.
- `src/ui/list_detail.rs:18-20` - the "reusable widget" re-exports `advance_highlight`, `first_occupied`,
  `next_index`, `prev_index` from `crate::ui::inventory_preview`. The widget depends on one of its consumers.
- `src/items/data.rs:196` - `EquipEffect::NoticeChance` documents itself against
  `crate::ai::decisions::BASE_NOTICE_CHANCE`; `src/combat/equip_stats.rs:431` reads that AI constant to fold
  a gear stat. Items → AI.
- `src/combat/mod.rs:491` - `sneak_mult(category: Option<crate::items::ItemCategory>)`. The combat core
  imports the item category enum to hard-code `Dagger → 5×`.

### B3. Where content leaks into engine-shaped code

Grep counts across `src/` (comments and tests included): Fire 514, spell 203, Poison 195, Lich 152, Wand 113,
mana 95, Goblin 95, Potion 90, Arrow 88, Dagger 87, Staff 85, Sword 83, Rat 70, Bow 60.

The good news: almost every one of those is a **doc comment or a test fixture**. Filtering to non-test
production code, the only hard-coded content *string* in the whole scope is:

```
src/actors/spawner.rs:1684:  const STARTING_ITEMS: &[(&str, bool)] = &[("Dagger", true)];
```

That is a genuinely strong result and the direct payoff of the RON-registry discipline.

The leak is at the **type** level instead, and it is pervasive:

| Type | Path | Why it is a leak |
|---|---|---|
| `DamageType { Physical, Fire, Cold, Poison }` | `src/combat/mod.rs:56` | `#[non_exhaustive]` but no `Custom` escape hatch. A sci-fi game needs Kinetic/Thermal/EMP/Radiation and cannot add them. |
| `StatusEffect` (17 variants) | `src/combat/effects.rs:148-235` | Closed. Includes `GoopyEyes` - a single monster's gimmick - in the *shared* vocabulary. |
| `AuthorableStatus` (16 variants) | `src/combat/effects.rs:253-282` | A hand-maintained near-duplicate of `StatusEffect` with a 1:1 `From` impl (`:284-305`). Two enums, one meaning. |
| `EquipEffect` (28 variants) | `src/items/data.rs:175-267` | Every stat the game has, as an enum variant, with a hand-written `merged_with` fold (`:289-332`) and a mirrored `EquipAccum` struct field. Adding one stat is a 4-site edit. |
| `ItemCategory` (Dagger/Sword/Axe/Hammer/Staff/Wand/Bow/Arrow/Head/Body/Arms/Feet/Shield/Ring) | `src/items/data.rs:35-96` | Pure fantasy taxonomy, and it is the *affix eligibility key* (`AppliesTo::Category`, `src/items/affixes.rs:62`) and the *sneak multiplier key* (`src/combat/mod.rs:491`). |
| `EquipSlot` (MainHand/OffHand/Head/Body/Arms/Feet/Ring1/Ring2) | `src/core/components.rs:470-486` | Closed; a sci-fi game wants implant slots, a naval game wants rigging slots. |
| `Faction { Player, Hostile }` | `src/core/components.rs:93-96` | Two variants. No third party, no neutrals, no relation matrix. `nearest_enemy` is literally `v.faction != my_faction` (`src/actors/ai/brain.rs:426`). |
| `TacticId` (15 variants) | `src/actors/ai/brain.rs:105-131` | Closed, with a hand-written string parser (`:149-172`) and a hand-written dispatcher (`try_tactic`, `:1062`). Adding a tactic is a 3-site edit and *impossible* from a game crate. |
| `AbilityEffect` (12 variants) | `src/combat/abilities.rs:85-150` | Closed; includes `RaiseDead`, `Berserk`, `Web`, `Summon`. |
| `MonsterTrait { HeatAura, ColdAura }` | `src/actors/monster_data.rs:290-293` | Two fantasy variants. |
| `CleanseKind { Poison, Bleed, Burn, Chill, Stun, All }` | `src/items/data.rs:116-123` | Mirrors the status enum a third time. |
| `HitKind { Strike, OffHand, Shot, Spell, Retaliate }` | `src/combat/narrate.rs:30 (HitKind)` | Fantasy verbs. |
| `ActiveModal` (16 variants) | `src/ui/mod.rs:70-92` | Every screen the game has, in one enum the "reusable" tab widget switches on. |

### B4. System-ordering contracts

This is the strongest part of the codebase's architecture and should be copied verbatim.

Three owners, documented and enforced (`CLAUDE.md` "System ordering is split across three owners"):
1. `src/main.rs` owns `AppState` gating and cross-plugin set nesting via `configure_sets`.
2. `TurnOrderPlugin` (`src/core/turns.rs`) owns the `ProcessingPhase` chain
   (Brain → ResolveMovement → ResolveActions → Cleanup) and the `TurnState::Processing` gate.
3. Plugins place their own systems into sets and may order *within* their module, but must not reorder
   another plugin's sets.

`CombatEventSet` (`src/combat/mod.rs:1325-1345`) explicitly documents that `CombatPlugin` does **not** set
`run_if` or cross-plugin ordering - the game does. That is exactly the right contract for an engine plugin,
and the doc comment even ships the `configure_sets` snippet.

`CombatPhase` (`src/combat/mod.rs:1345-1372`) collapses what the header describes as "~22 per-system ordering
edges" into three chained sets, with a first-class explanation of *why* the phases exist (a 2-frame message
buffer would expire between Processing frames and silently drop DoT damage). This is high-quality
engineering.

Residual leaks from the clean model, all of them cross-plugin `.after(concrete_system_name)` edges:
- `src/combat/mod.rs:1546-1553` - `constrict_release_system.after(lifecycle::process_deaths)`
- `src/combat/mod.rs:1500-1502` - `explode_on_death → explosion → burn_webs` chained by name
- The doc at `:1362-1366` admits `AbilityPlugin` and `LevelingPlugin` order against concrete public systems.

Each of those is a missing phase. An engine should expose enough named phases that no downstream plugin ever
names a concrete system.

### B5. Event / message patterns

`#[derive(Message)]` + `MessageWriter`/`MessageReader` is used consistently and makes a good engine seam.
`DamageEvent` (`src/combat/mod.rs:1195-1240`) is the model citizen: 10 fields, every one documented, and it
draws two distinctions most damage systems get wrong -

- `attacker: Option<Entity>` = who triggers on-hit riders (None for DoT/splash, so riders can't recurse)
- `credit: Option<Entity>` = who gets kill credit (Some even for DoT, so XP still lands)

That split is a real design insight and should ship in the engine.

Problems:
1. **`damage_application_system` is a monolith.** 615 lines, and it is at Bevy's 16-parameter ceiling with
   two params being *tuples of six queries each* (`reaction_writers` at `:1584-1591`, `combat_q` at
   `:1601-1625`) purely to dodge the limit. It handles: distance modifiers, empower/weaken/berserk scaling,
   godmode, resistance, shield block + on-block riders, armor, ward/vulnerable, narration, on-hit riders
   (8 variants), automatic thermal riders, trap narration, HP subtraction, regen lockout, on-defend riders
   (5 variants), thorns second pass, cleave second pass, attacker-status second pass, and death emission.
2. **Prose-matched log colouring.** `src/ui/game_log.rs:145-181` classifies a message by
   `s.contains("hits you") || s.contains("shoots you") || …` over ~30 English substrings. The log message is
   a `String`; its *semantics* were thrown away at the write site and are reconstructed by grep. Any engine
   log must carry a category/severity on the message.
3. `GameLogMessage(String)` is the only log channel. Every system that wants to say something formats English
   at the emit site.

---

## C. Traits and extension points

### C1. Traits that exist and work

| Trait | Path | Assessment |
|---|---|---|
| `TimedStatus` | `src/combat/mod.rs:2216-2221` | **The right idea.** `turns_remaining()` + `tick()`, driving one generic `expire_status::<T>` system (`:2276-2297`) registered 11 times (`:1530-1543`). |
| `DotStatus` | `src/combat/mod.rs:2223-2234` | Same shape plus `damage_per_turn`/`damage_type`/`source`/`ignores_armor`, driving `tick_dot::<T>` (`:2299-2335`). Currently used for exactly **one** type (`Burning`, `:2268-2270`) - the abstraction is under-exploited. |
| `BaseMap` / `Algorithm2D` (bracket-lib) | used by `HazardMap`, `src/actors/ai/pathfinding.rs:80-160` | **Best trait usage in the repo.** `HazardMap` wraps a `&Map` and overrides only `get_available_exits` to add a fire penalty scaled by the pather's resistance, delegating geometry unchanged. This is exactly the cost-overlay decorator an engine should ship. |
| `Component<Mutability = Mutable>` bound | `src/combat/mod.rs:2276` | Correct use of Bevy 0.17 mutability bounds. |

### C2. Traits taken as parameters

Rare but present and effective:
- `accumulate_equip_effects(base_resist, items: impl Iterator<Item = (&ItemDef, &[EquipEffect])>)`
  (`src/combat/equip_stats.rs:189-192`). One accumulator serving both the player recalc and monster spawn.
  This is the single best "one code path, two callers" seam in the codebase and directly implements the
  CLAUDE.md invariant.
- `trace_shot(from, tx, ty, map, hostile_at: impl Fn(i32,i32) -> Option<Entity>)`
  (`src/combat/archery.rs:120-147`). Closure-injected occupancy, so archery and throwing share one answer to
  "where does a projectile stop". Pure, 27 lines, immediately extractable.

### C3. `#[non_exhaustive]` + `Custom { id }` vs. the alternatives - what each repo does

- **fantasy-rogue**: closed enums everywhere. `DamageType` is `#[non_exhaustive]` (`src/combat/mod.rs:51`) but
  that only helps *the crate that owns it* add variants. A downstream game crate still cannot add `Radiation`.
  This is the fundamental blocker for theme-agnosticism in this codebase.
- **roguelike_engine**: `WorldStateProp` is `#[non_exhaustive]` **with** a `Custom { id: u32 }` variant and a
  `custom: BTreeMap<u32, bool>` on `WorldState` (`/Users/nathanrude/Development/roguelike_engine/src/ai/goap.rs:76-99`).
  Downstream games can extend. **But** the blessed field set is polluted with one specific game's content:
  `at_hoard`, `adjacent_to_chest`, `carrying_items` are commented `// --- Kobold hoarder ---`
  (`goap.rs:57-62`). The escape hatch exists and the engine still leaked a kobold into it. That is the single
  most instructive data point for the theme question.
- **Verdict**: `Custom { id: u32 }` is better than nothing but is a second-class citizen - a `u32` with no
  name, no serde round-trip to RON, and no way to give it a badge/colour/description. For content taxonomies
  (damage types, statuses, item categories, slots, factions, tactics) prefer either an interned string id
  backed by a game-registered table, or a generic type parameter.

### C4. The registry pattern - a `ContentRegistry<T>` is sitting right there

Eleven parse seams, one shape:

```
src/combat/abilities.rs:385   load_abilities_from_str      -> AbilityRegistry(HashMap<String, AbilityDef>)
src/items/data.rs:810         load_items_from_str          -> ItemRegistry(HashMap<String, ItemDef>)
src/items/data.rs:1114        load_item_spawns_from_str    -> spawn table
src/items/affixes.rs:220      load_affixes_from_str        -> AffixRegistry(BTreeMap<String, AffixDef>)
src/items/loot_pools.rs:89    load_loot_pools_from_str     -> LootPoolRegistry(HashMap<String, Vec<..>>)
src/items/item_class.rs:123   load_item_class_weights_from_str
src/actors/monster_data.rs:862 load_monsters_from_str      -> MonsterRegistry { defs: HashMap<..> }
src/actors/monster_data.rs:881 load_spawn_rules_from_str   -> SpawnRules { entries: Vec<SpawnEntry> }
src/map/prefab.rs:198         load_prefabs_from_str
src/map/props/mod.rs:104      load_props_from_str
src/map/props/spawn.rs:89     load_scatter_table_from_str
```

Every one of them is followed by an identical plugin:

```rust
const MONSTERS_RON: &str = include_str!("../../assets/monsters.ron");
pub fn load_monster_data(mut commands: Commands) {
    let registry = load_monsters_from_str(MONSTERS_RON)
        .unwrap_or_else(|e| panic!("Failed to parse assets/monsters.ron: {e}"));
    commands.insert_resource(registry);
}
```
(`src/actors/monster_data.rs:832-849`)

**A generic `ContentRegistry<T: DeserializeOwned>` with `Resource` + `Deref<Target = HashMap<DefId, T>>`, a
`from_ron_str` seam, and a `validate(&self) -> Result<(), Vec<String>>` hook would replace all eleven.**
Note `load_monsters_from_str` (`:862-878`) already demonstrates the validate hook: it rejects
`Intellect::Mindless` combined with `flee_at_hp > 0` because that combination describes a creature that
cannot exist. That "semantic validation on top of parse" is worth a first-class trait.

`SpawnRules::entries_for_floor` (`src/actors/monster_data.rs:807-818`) plus the weight sampling in the spawner
is a second generic: a **`BandedWeightedTable<T>`** (`min_depth`, `max_depth`, `weight`, `min_group`,
`max_group`) that monsters, items, props and scatter all reimplement separately today.

### C5. Traits that SHOULD exist and don't

1. **`Stat` / modifier stack.** `EquipEffect` (28 variants) → `EquipAccum` (25 named fields) →
   `merged_with` (28 match arms) → `EquipAccum::apply` (28 more) → the recalc's per-stat component writes.
   Adding one stat touches five places. An engine wants
   `Modifier { stat: StatId, op: Add | Mul | Max, value: f64 }` over a game-registered `StatId` table, with
   one generic accumulator. The comment at `src/items/data.rs:284-288` even records that the two folds
   already drifted apart once and were only fixed by making both matches exhaustive - that is the
   abstraction asking to be built.
2. **`StatusKind`.** Today a status = a component + an `apply_status` arm + a tick/expire registration + a
   `status_view::badges` arm + a `CleanseKind` arm + an `AuthorableStatus` arm + a `From` arm. Seven sites.
   A `StatusDef { id, duration_semantics, per_turn_effect, modifiers, badge }` registry with one generic
   apply/tick/expire trio would collapse all of it. The memo referenced in the repo's memory
   ("status-effect-architecture-decision", generic-system-over-components) chose the current model
   deliberately; for a *library* the trade-off flips, because the engine cannot enumerate the game's statuses.
3. **`Faction` / relation.** Replace the two-variant enum with `FactionId(u16)` plus a
   `FactionRelations` matrix (hostile/neutral/allied), and take it as a parameter to targeting.
4. **`DamageResolver` / mitigation chain.** `damage_application_system` hard-codes the order
   resist → block → armor → ward/vulnerable. Make each stage a trait object or a typed pipeline the game
   composes; that alone would cut the 615-line system into six testable stages.
5. **`Tactic` trait.** `fn evaluate(&self, ctx: &mut TacticCtx) -> Option<Action>` as a boxed trait object in
   a `Vec`, replacing `TacticId` + `parse_tactic` + `try_tactic`. The `TacticCtx` struct
   (`src/actors/ai/brain.rs:914-965`) is already the right shape - it just needs to be `pub` and the dispatch
   needs to be open.
6. **`ContentId`.** Every registry is keyed by `String` and every def-reference is a `String` (`MonsterDefId`,
   `ItemDefId`, `AbilitySlots(Vec<String>)`, `Cooldowns(HashMap<String, u32>)`). String hashing on every AI
   cooldown check. An interned `Id<T>(u32)` with a `PhantomData<T>` would be both faster and type-safe -
   today nothing stops passing a monster id where an ability id is expected.
7. **`Narrator`.** `narrate::hit_line` is the right seam in the wrong crate (see F7).
8. **`ThreatModel`.** The balance checker's `estimate_threat` (`src/actors/balance.rs:864`) takes
   `&MonsterDef, &MonsterRegistry, &ItemRegistry, &AbilityRegistry, &LootPoolRegistry`. Give it a trait
   (`fn effective_hp(&self) -> f64`, `fn dps(&self) -> f64`, `fn hd(&self) -> u32`) and the whole HD-scoping /
   floor-progression / outlier machinery becomes theme-free.

---

## D. Performance review

Turn-based, so the frame budget is generous and none of this is currently a problem *for this game*. All of
it matters for a library that must not force the same shape on the next game.

### D1. No spatial index. This is the single biggest structural perf issue.

There is **no entity-at-tile index anywhere in the codebase** (`grep -rn 'fn actor_at\|entity_at' src/` → 0).
Every "who is standing there" query is a linear scan over an actor query:

- `resolve_affected` (`src/combat/abilities.rs:1325-1378`) scans **all actors** for every AOE cast, four times
  over (one arm per `AoeShape`), building a `HashSet<Point>` footprint and testing `footprint.contains(pos)`.
- `handle_movement`'s occupancy check scans `Query<(Entity, &Position, &Faction)>` per move
  (`src/combat/actions.rs:794`).
- Cleave scans `Query<(Entity, &Position, &Faction), With<Health>>` per cleaving hit
  (`src/combat/mod.rs:1608`).
- `explosion_system`, `burn_webs_system`, `trace_shot`'s `hostile_at` closure - all the same.

`Map.blocked: Vec<bool>` (`src/map/map.rs:34`) exists but stores only a bool and, per its own doc comment at
`:25-26`, "is recomputed every frame". It also famously excludes the player (the repo's memory records the
door-close bug this caused), so nothing can rely on it for occupancy.

**Recommendation:** the engine ships a `SpatialGrid { cells: Vec<SmallVec<[Entity; 2]>> }` updated on
position change (or once per turn), exposing `at(x, y) -> &[Entity]` and `in_radius(center, r)`. Every one of
the scans above becomes O(footprint) instead of O(actors).

### D2. Per-monster A*, no shared flow field

`step_toward_cached` (`src/actors/ai/pathfinding.rs:257-293`) runs a full `a_star_search` per monster on cache
miss. `PathCache` (`:208-249`) is a good mitigation - it probes `idx`, `idx±1` for an O(1) hit and only
recomputes when the monster is knocked off-path or a cached tile catches fire. But every hunting monster
still owns an independent A* to the same target.

The codebase already *has* the better tool and uses it elsewhere: `DijkstraMap` appears in
`src/map/builders/stairs.rs:127` (stair placement) and `src/actors/auto_explore.rs:331` (frontier explore),
but never in the AI. One Dijkstra map seeded at the player, recomputed once per turn, would serve every
hunting monster with a single O(cells) pass and a per-monster O(8) descent. That is the standard roguelike
answer (Brogue's "safety maps", DCSS travel).

Caveat: the hazard-aware pather (`HazardMap`) is per-monster (`heat_factor` is the individual's fire
resistance, `can_open_doors` its intellect). A flow field would need one map per *class* of pather - which is
still 2-3 maps instead of N.

### D3. `HashSet<Point>` in hot paths

`Viewshed.visible_tiles: HashSet<Point>` (`src/core/components.rs:656`). `nearest_enemy`
(`src/actors/ai/brain.rs:419-434`) does a `visible.contains(&v.pos)` per candidate per monster per turn.
Rendering, examine, targeting, and auto-explore all hit the same set. A `FixedBitSet` (or `Vec<u64>`) over
the grid index would be ~1 word per 64 tiles, cache-friendly, and give O(1) with no hashing. On an 80×50 map
that is 63 words vs. a hash set of up to ~700 `Point`s.

`burst_footprint` / `cone_footprint` / `line_footprint` all return `HashSet<Point>` or `Vec<Point>` allocated
fresh per cast (`src/combat/abilities.rs:1273-1424`).

### D4. Per-frame allocation in the brain

`monster_ai_brain` builds two `Vec`s at the top of *every* invocation, before the monster loop:
`entity_views` (`src/actors/ai/brain.rs:513-527`) over all faction-bearing entities, and `corpse_views`
(`:530-546`) over all corpses. These are rebuilt on every Processing frame even when the `(Monster, MyTurn)`
query is empty. Cheap in absolute terms, but it is exactly the pattern the engine should not enshrine - the
snapshot should be a resource refreshed once per turn.

### D5. String-keyed lookups in AI decisions

`Cooldowns(HashMap<String, u32>)` (`src/combat/abilities.rs:409`), `AbilitySlots(Vec<String>)` (`:422`).
`best_cast` (`src/actors/ai/targeting.rs`) iterates the caster's abilities, hashing a `String` per ability per
scoring pass, then scores against every legal target over the AOE footprint - so string hashing sits inside a
double loop. Interned ids fix this for free.

### D6. Benchmarks

**None in fantasy-rogue.** No `benches/` directory, no criterion dependency (`Cargo.toml` has bevy,
bracket-lib, petgraph, rand, ron, serde, web-sys and nothing else). roguelike_engine has criterion benches;
that is one thing it does better and the new engine must keep.

The two `examples/` (`balance_report.rs`, `xp_pacing.rs`) are *analysis* tools, not benchmarks - but they are
excellent as examples and prove the parse seams are usable from outside.

### D7. Per-frame systems

`update_blocked` (`src/map/map.rs:255-272`, sibling's scope) runs every frame and rewrites the whole
`blocked` vector. In-scope consumers assume it is fresh. `damage_application_system` and the tick systems are
all correctly gated behind `CombatPhase` inside `CombatEventSet` inside `ProcessingPhase`, which is itself
`run_if(in_state(TurnState::Processing))` - that gating discipline is good and should be an engine rule.

---

## E. Quality assessment

### E1. Tests - genuinely strong

1,577 `#[test]` functions in `src/`, no `tests/` directory (everything is `#[cfg(test)]` in-module). Roughly
1:1 test-to-production LOC in the big files (combat/mod.rs 3,208 prod / 4,412 test; equip_stats.rs
628 prod / 1,776 test).

Four distinct kinds, all present:
- **Guard tests over live assets.** `every_monster_gear_and_ability_id_resolves`
  (`src/actors/balance.rs:1659`), `every_real_castable_staff_resolves_its_ability`
  (`src/combat/abilities.rs:1937`), `every_status_maps_to_its_badge` (`src/combat/status_view.rs:228`),
  `every_item_class_covers_every_floor`, `every_floor_has_at_least_one_eligible_prefab`
  (`src/map/prefab.rs:2216`). These read the *embedded* RON, so a content edit that breaks an invariant fails
  CI. Best practice; the engine should ship the harness for it.
- **Property-over-seed-range.** `every_floor_of_every_seed_keeps_its_exits` (`src/map/generate.rs:349`).
- **Headless `App` tests.** ~80 across the tree (`spawner.rs` 19, `handlers.rs` 13, `components.rs` 11,
  `combat/mod.rs` 7, `abilities.rs` 6). This is what makes the plugin seams verifiable.
- **Pure unit tests** on the arithmetic and decision helpers.

Round-trip RON parse tests (`prop_effect_round_trips_all_v1_variants_from_ron`,
`src/combat/effects.rs:513`) close the schema loop.

Gap: no fuzz/proptest crate, no benchmark, and the guard tests read *live* assets, which the repo's own memory
flags as concurrent-edit sensitive.

### E2. Documentation - the best of the three repos

Every module has a substantive `//!` header that explains *why*, not just what. Standouts:
- `src/combat/narrate.rs:1-28` - explains the bug that motivated the module (log announced pre-mitigation
  damage while the health bar showed post-mitigation) and what the seam therefore guarantees.
- `src/combat/mod.rs:1345-1372` - `CombatPhase` explains the 2-frame message-buffer hazard that forced the
  phase split.
- `src/combat/phylactery.rs:1-25` - explains why the pairing is by `pair_id` and not by def id, citing the
  concrete failure (multiple Lich pairs on one floor).
- `src/items/enchant.rs:1-30` - "There is exactly ONE source of truth per number" with the three call sites.
- `src/actors/ai/gear.rs:1-13` - documents that the module exists *because* the brain is at the 16-param
  ceiling. Honest about a workaround.

Weaknesses:
- **No `#![deny(missing_docs)]`.** `src/lib.rs:1-8` only sets
  `#![allow(clippy::too_many_arguments, clippy::type_complexity)]` - the two lints that would have caught the
  param-ceiling problem, silenced crate-wide. Defensible for a game; unacceptable for a library.
- **9 doc-tests** across 92k LOC, and the main plugin-usage example is `` ```ignore ``
  (`src/combat/mod.rs:1334-1342`). Doc-tests that don't run are documentation that can rot.
- The crate-root re-export block (`src/lib.rs:29-36`) preserves historical paths so `crate::components`,
  `crate::actions`, `crate::ai` all resolve regardless of folder. Convenient, but it means the folder
  structure carries no enforcement - anything can reach anything.
- `examples/` has 2 entries, both analysis reports. No "here is how you use this" example.

### E3. Determinism - the best model in any of the three repos

`RunSeed` + `SeedDomain::derive(domain, floor)` (`src/core/rng.rs:81-126`). Nine domains
(Map, Prefabs, PrefabContent, Monsters, Items, Props, ItemScatter, Combat, Hordes), each with a distinct odd
salt constant. The doc comments explain *why* each split exists - `PrefabContent` is separate from `Prefabs`
because terrain stamps at generation while content spawns on first visit, so a shared stream would let one
half's draw count shift the other's on revisit (`:87-92`). `ItemScatter` is separate from `Props` because
they share a table and a loop, and retuning trap density would otherwise reshuffle every rock (`:99-104`).

Three separate RNG streams with a documented invariant (CLAUDE.md, "RNG determinism is an invariant"):
- `GameRng` - one shared bracket-lib stream for combat, reseeded per run from `SeedDomain::Combat`
- per-entity `StdRng` seeded from `entity.index()` for AI idle movement
- per-floor `StdRng` for spawn placement, affixes, enchant rolls
- `FxRng` - cosmetic only, deliberately never reseeded

The discipline is enforced at call sites with explicit comments (`src/items/enchant.rs:25-29`,
`src/combat/mod.rs:1578-1583` on the isolated thermal RNG). One hash-based escape hatch exists and is
justified: `notice_roll` (`src/actors/ai/brain.rs:407-413`) hashes `(entity.index(), turn_time, salt)` rather
than drawing from a stream, because a per-entity stream would give a *constant* seed across turns for an
unaware monster - so it would either notice you on turn one every time, or never. Reproducible, per-turn
varying, touches no shared stream. Good reasoning.

**This model should be lifted wholesale into the engine**, with `SeedDomain` made extensible
(games add domains) rather than a closed enum.

### E4. Notable bugs and smells spotted in passing

1. **`handle_movement` contains the melee system.** `src/combat/actions.rs:777-1403`, 626 lines. No
   `AttackIntent` exists. A monster with a stationary movement mode and an adjacent enemy attacks via
   `TacticId::MeleeAdjacent` writing a `MovementIntent` into an occupied tile. Attacking is a side effect of
   trying to walk.
2. **Narration assumes the definite article.** `narrate::strike_clause` (`src/combat/narrate.rs:142-152`)
   always emits `"The {target}"`. A named monster ("Grishnak the Bold") reads as "The Grishnak the Bold".
   `assets/monsters.ron` has an `epithet` field, so named uniques are an intended direction.
3. **`AuthorableStatus` duplicates `StatusEffect`** with a 16-arm hand-written `From`
   (`src/combat/effects.rs:297-320`). The doc at `:243-262` explains that the restriction it once enforced
   (banning Burn/Chill/Freeze) has *moved* to `apply_status`. The type now restricts nothing - it is a
   maintenance liability with no invariant left to protect.
4. **Two-tuple params to dodge the 16-param ceiling** appear in at least four systems
   (`damage_application_system` ×2, `monster_ai_brain` ×2 plus a nested `QueryData` tuple at
   `src/actors/ai/brain.rs:464-472`, `handle_movement`, `recalculate_stats`). `src/actors/ai/gear.rs` exists
   *solely* to precompute one answer so the brain doesn't need two more params. The lint that would flag this
   (`clippy::too_many_arguments`) is allowed crate-wide at `src/lib.rs:8`. The systems are too big; the tuples
   hide it.
5. **`DotStatus` is implemented once.** `src/combat/mod.rs:2268-2270` - only `Burning`. `Poisoned` can't use
   it (stacks multiple instances, `:2537`), `Bleeding` has its own system (`:2597`), `Constricted` has its own
   (`:2771`). Four DoTs, one trait, three bespoke systems.
6. **`AbilityDef::monster_only`** is documented dead (`src/combat/abilities.rs:180-186`) - its only reader was
   deleted in Phase 20, kept by an explicit decision. `AbilityDef::level` is "inert flavor/tier metadata"
   (`:159-161`). `AiDef::tactics` is authored-but-partially-read per `src/actors/monster_data.rs:14-17`.
   `MoveRepeat` is "a no-op marker kept for spawner compatibility" (`src/actors/player.rs:12-16`).
   `src/items/stats.rs:9-16` documents itself as a mostly-deleted module. Small, but it is drift.
7. **`ExplosionEvent` uses Chebyshev radius** (`src/combat/mod.rs:1272`) while `Burst` AOE uses a
   wall-bounded circular footprint (`burst_footprint`, `src/combat/abilities.rs:1273`). An on-hit `Explode`
   and a Fireball of the same radius cover different tiles and one ignores walls.
8. **Godmode branch inside the damage loop.** `src/combat/mod.rs:1698-1726` - a cheat check with its own
   narration path, in the hottest combat system, gated on an `Option<Res<CheatState>>`.

---

## F. Recommendations for the new engine (ranked)

### F1. Ship the seed-domain determinism model as a first-class engine crate. (Highest confidence.)
**What:** `RunSeed`, `SeedDomain::derive(domain, floor)`, and the three-stream separation.
**Start from:** `/Users/nathanrude/Development/fantasy-rogue/src/core/rng.rs:81-140`.
**Change:** make the domain set extensible. `SeedDomain` becomes a trait or a newtype over a `u64` salt that
games register (`const COMBAT: SeedDomain = SeedDomain::new(b"COMBAT")`), so a game can add
`SeedDomain::Weather` without editing the engine. Keep the doc-comment discipline that explains *why* each
domain is split - that is what makes the invariant survive contact with contributors.

### F2. Damage pipeline as a composable mitigation chain, not one 615-line system.
**What:** `DamageEvent` → ordered stages → `Health` → `DeathEvent`.
**Start from:** `src/combat/mod.rs:1195-1240` (the event, which is already excellent - keep the
`attacker`/`credit` split verbatim) and `CombatPhase` (`:1345-1372`).
**Change:** decompose `damage_application_system` into named stages the game composes -
`ScaleByAttackerModifiers`, `ApplyResistance`, `TryBlock`, `SubtractArmor`, `ScaleByDefenderModifiers`,
`Commit`, `RunHooks`. Each stage is a trait impl or an ordered system in a `DamageStage` set. This kills the
16-param tuples, makes each stage unit-testable, and lets a sci-fi game insert `ApplyShieldRecharge` between
two of them without forking the engine. Move the second-pass collections (thorns / cleave / attacker-status)
onto a first-class **reaction queue** type instead of five game-owned `Message`s the engine plugin has to
register.

### F3. Extract a real modifier/stat stack; delete `EquipEffect` and `EquipAccum` as enums.
**What:** `StatId` (game-registered), `Modifier { stat, op: Add|Mul|Compound|Max, value }`, one generic
accumulator with a `merge` rule per op.
**Start from:** `src/combat/equip_stats.rs:189-226` (`accumulate_equip_effects` - the *seam* is right: it
takes an iterator and serves both player and monster) and `src/items/data.rs:269-333` (`merged_with` - the
*semantics* are right: everything sums except the two multipliers, which compound).
**Change:** the 28-variant enum and its 25-field mirror struct become one `HashMap<StatId, StatValue>` or a
dense `Vec<f32>` indexed by a registered stat table. The comment at `src/items/data.rs:284-288` documents that
the two folds already drifted once - that is the abstraction paying for itself before it exists.
This is the "engine-grade effect stack hiding in here" the brief asks about: **yes, it is there, and it is
`accumulate_equip_effects` + `merged_with`, not `EquipAccum`.**

### F4. Generic `ContentRegistry<T>` + `BandedWeightedTable<T>`.
**What:** one registry type replacing eleven, with a `from_str` seam, a `validate()` hook, and a
`Deref<Target = HashMap<Id<T>, T>>`.
**Start from:** `src/actors/monster_data.rs:832-878` (loader + the semantic-validation example) and
`src/actors/monster_data.rs:807-818` + `SpawnEntry` for the banded table.
**Change:** key on an interned `Id<T>(u32)` rather than `String` (type safety + no hashing in AI inner loops).
Keep `include_str!` embedding as an option (it is what makes WASM builds and offline analysis tools work) but
also support runtime asset loading. Ship the **validate-on-load** idea prominently - it is the difference
between a typo'd RON file failing at boot with a sentence and failing three floors down with a missing
monster.

### F5. Open the AI: keep the tactic-priority architecture, drop GOAP.
**What:** an ordered list of `Box<dyn Tactic>` where `Tactic::evaluate(&self, ctx: &TacticCtx) -> Option<Action>`,
first match wins.
**Start from:** `src/actors/ai/brain.rs:979-1016` (`dispatch_tactics`), `:914-965` (`TacticCtx`),
`:207-262` (`tactics_for` - how knobs compose a default list), and `src/actors/ai/decisions.rs` entire.
**Change:** `TacticId` (closed enum) + `parse_tactic` (string match) + `try_tactic` (dispatcher) collapse into
a game-registered `TacticRegistry: HashMap<Name, Arc<dyn Tactic>>`. `TacticCtx` becomes `pub` and gains a
`&dyn Any` game-extension slot. Fix the param ceiling by having the brain read a per-turn snapshot resource
rather than 16 queries - which also deletes the reason `src/actors/ai/gear.rs` exists.

**On GOAP specifically:** roguelike_engine's `plan()` is a forward-chaining A* over `WorldState` with a depth
limit of 4 (`/Users/nathanrude/Development/roguelike_engine/src/ai/goap.rs:285`). It is more expensive, harder
to debug ("why did it do that?" requires replaying the plan), and its blessed `WorldState` leaked one game's
content into the engine (`at_hoard`, `adjacent_to_chest`, `goap.rs:57-62`). fantasy-rogue's priority list
produces behaviour that is at least as rich (kite, flee, drink, re-equip, throw, fetch, cast, patrol,
follow-summoner) at O(tactics) per turn with a trivially readable trace. **Ship the priority list as the
default and GOAP as an optional crate**, not the other way round.

### F6. Ship a spatial index and a shared flow field. These are the two real perf wins.
**What:** `SpatialGrid` (`at(x,y) -> &[Entity]`, `in_radius`) + a per-turn `DijkstraMap` toward the player /
toward arbitrary goals.
**Start from:** nothing in fantasy-rogue - it has neither. `src/actors/auto_explore.rs:331` shows the
`DijkstraMap` usage pattern; `src/actors/ai/pathfinding.rs:80-160` (`HazardMap`) shows the cost-overlay
decorator to keep.
**Change:** every AOE resolution, cleave, explosion, projectile occupancy test and movement check in this
codebase is currently an O(actors) scan (`src/combat/abilities.rs:1325-1378` is the worst). Also replace
`Viewshed.visible_tiles: HashSet<Point>` with a bitset - it is read in the AI inner loop
(`src/actors/ai/brain.rs:429`). Add criterion benches for FOV, A*, Dijkstra, and AOE footprint from day one;
fantasy-rogue has none and roguelike_engine's are the one thing to carry forward from it.

### F7. Make the log semantic, and put narration behind a trait.
**What:** `LogMessage { category, severity, payload }` where payload is structured, plus a
`trait Narrator { fn render(&self, event: &LogEvent) -> String }` the game implements.
**Start from:** `src/combat/narrate.rs` entire - it is the right *idea* (compose the sentence at the one point
where the true damage is known, and let attack sites attach only what the pipeline cannot infer). The
`HitView` / `HitNarration` split is exactly the engine/game boundary.
**Change:** `src/ui/game_log.rs:145-181` classifies log lines by matching ~30 English substrings
(`s.contains("hits you")`). That is the direct consequence of throwing the semantics away at the write site.
The engine emits typed events; the game's `Narrator` turns them into text; the log colours by category. This
also makes localization possible, which the current design forecloses.

### F8. Split `handle_movement`: `MoveIntent` and `AttackIntent` are different actions.
**What:** a separate attack resolution system.
**Start from:** `src/combat/actions.rs:777-1403`.
**Change:** bump-to-attack becomes a *player input convenience* that emits `AttackIntent` when the
destination is occupied by a hostile - not 400 lines of combat inside the movement handler. This unblocks
reach weapons, attacks-without-moving, monster ranged melee, and testing attack resolution in isolation. It
is the single clearest structural defect in the scope.

### F9. Keep the `SystemSet` ownership contract exactly as written.
**What:** the three-owner rule (app owns state gating + cross-plugin set placement; the turn plugin owns the
phase chain; plugins own placement within their own sets and never reorder another's).
**Start from:** `CLAUDE.md` "System ordering is split across three owners", `src/combat/mod.rs:1329-1372`,
`src/core/turns.rs` `ProcessingPhase`.
**Change:** expose *more* named phases so no downstream plugin ever needs
`.after(concrete_system_name)`. Today `src/combat/mod.rs:1500-1502` and `:1546-1553` still chain by system
name, and the doc admits two external plugins do the same. Each such edge is a phase the engine forgot to
name. Also: an engine plugin must never register another module's messages for test convenience
(`src/combat/mod.rs:1382-1412`) - give the engine a `TestApp` builder instead.

### F10. Ship the affix / enchant model nearly as-is - it is the strongest content system here.
**What:** `AffixDef { name, kind, applies_to, equip_effects, on_hit, on_defend, on_block, scaled_* }` plus
per-instance `+N` with a data-driven `EnhanceRule`.
**Start from:** `src/items/affixes.rs:167-200` (`AffixDef`), `:79-160` (`ScaledOnHit`/`ScaledEquipEffect` -
base-plus-per-level, with the documented reasoning that a pure multiplier makes a `+0` item do nothing), and
`src/items/enchant.rs:127-175` (`EnhanceRule`, with the one-source-of-truth-per-number rule).
**Change:** `AppliesTo::Category(ItemCategory)` must key on a game-defined tag set (`applies_to: [Tag]`,
tags interned from RON) rather than a fantasy enum. Everything else generalizes untouched.

### F11. Generalize the balance checker into an engine analysis tool.
**What:** `Threat = EffectiveHP × DPS`, HD-scoping outliers, per-depth spawn progression, spawn-row outliers,
driven off the *live* content through the same parse seams the game boots from.
**Start from:** `src/actors/balance.rs` entire + `examples/balance_report.rs`.
**Change:** define it over a `trait ThreatSubject { fn effective_hp(&self, ctx) -> f64; fn dps(&self, ctx) ->
f64; fn tier(&self) -> u32; }` that the game implements for its own def type. The scoring *machinery*
(`fit_c`, `hd_scoping`, `floor_progression`, `spawn_outliers`, `src/actors/balance.rs:1189-1430`) is entirely
theme-free once the subject is a trait. **No other repo in the set has anything like this and it is a
genuinely differentiating engine feature** - "tune your content and immediately see the difficulty curve" is
the kind of tooling that sells a library.

### F12. Factions as a relation matrix, not a two-variant enum.
**Start from:** `src/core/components.rs:93-96` (what not to do) and `src/actors/ai/brain.rs:419-434`
(`nearest_enemy` - the only consumer, and it is one line of `!=`).
**Change:** `FactionId(u16)` + `FactionRelations` (hostile/neutral/allied, symmetric or not) + targeting
predicates taken as parameters. The current design cannot express a third party, neutrals, charmed monsters,
or a Robin Hood game's sheriff-vs-outlaws-vs-villagers at all.

### F13. Statuses as data, with the `TimedStatus`/`DotStatus` generic systems kept.
**Start from:** `src/combat/mod.rs:2216-2335` - `TimedStatus`, `DotStatus`, `expire_status::<T>`,
`tick_dot::<T>`. The generic-system idea is right.
**Change:** today a status costs seven edit sites (component, `apply_status` arm, tick/expire registration,
badge arm, `CleanseKind` arm, `AuthorableStatus` arm, `From` arm). For a *game* that trade-off was reviewed
and accepted; for a *library* it is fatal, because the engine cannot enumerate the game's statuses. Replace
with a `StatusDef` registry (duration, stacking rule, per-turn effect, stat modifiers, badge glyph/colour)
and one generic apply/tick/expire trio. Delete `AuthorableStatus` - the doc at
`src/combat/effects.rs:238-252` already records that the invariant it protected moved elsewhere.

### F14. `#![deny(missing_docs)]`, running doc-tests, and a real `examples/` directory.
**Start from:** the module-`//!` culture in `src/combat/narrate.rs`, `src/combat/mod.rs:1345`,
`src/items/enchant.rs:1-30` - this is already the best-documented of the three repos and the standard to hold.
**Change:** `src/lib.rs:8` allows `too_many_arguments` and `type_complexity` crate-wide, which is exactly the
signal that would have caught the param-ceiling problem. An engine crate should deny missing docs, forbid the
blanket allow (allow per-system where genuinely needed), and turn the `` ```ignore `` plugin examples
(`src/combat/mod.rs:1334-1342`) into compiled doc-tests. Nate's criterion #5 asks for "well documented with
use cases and a runnable examples/ directory" - fantasy-rogue has 2 examples, both analysis tools, and zero
"how to use this" examples.

### F15. The theme question: themes live in the game, but the engine must ship the *vocabulary machinery*.
**My position: no themes baked into the engine. But "theme-agnostic" is not achieved by removing content -
it is achieved by making every content taxonomy a registry.**

The evidence from these three repos is unusually clear:

- fantasy-rogue proves the **content-in-RON** half. Exactly one hard-coded content string survives in 62.6k
  lines of in-scope production code (`src/actors/spawner.rs:1684`). Monsters, items, abilities, affixes,
  props, prefabs, loot pools and spawn bands are all data. That discipline works and must be kept.
- fantasy-rogue also proves the **taxonomy-in-code** half fails. Thirteen closed enums
  (§B3) hard-code the fantasy taxonomy into types a downstream game cannot extend: `DamageType`,
  `StatusEffect`, `EquipEffect`, `ItemCategory`, `EquipSlot`, `Faction`, `TacticId`, `AbilityEffect`,
  `MonsterTrait`, `CleanseKind`, `HitKind`, `ArmorSlot`, `ActiveModal`. A sci-fi game reusing this engine
  cannot add `DamageType::Radiation` or `EquipSlot::NeuralImplant` without forking.
- roguelike_engine proves the **`Custom { id }` escape hatch is necessary but not sufficient**. It has the
  hatch (`WorldStateProp::Custom { id: u32 }`) and *still* leaked a kobold's chest-hoarding into the engine's
  blessed field set (`goap.rs:57-62`, literally commented `// --- Kobold hoarder ---`). If the blessed set
  exists, content will end up in it.

So the split I would draw:

| Engine ships | Game ships |
|---|---|
| The *shape* of a damage type (id, name, badge label, resistance ladder) | Which damage types exist |
| The *shape* of a status (duration, stacking, per-turn effect, modifiers) | Which statuses exist |
| The *shape* of a stat modifier and the accumulator | Which stats exist |
| The *shape* of an equipment slot graph (occupancy, two-handed claiming, dynamic resolution) | Which slots exist |
| Hook points: on_hit / on_defend / on_kill / on_death / on_block / damage_mods | What each hook does |
| Targeting: `TargetMode`, `AoeShape`, footprint resolution, LOS/wall bounding | Which abilities exist |
| Tactic dispatch, `TacticCtx`, the pure decision helpers | Which tactics exist |
| Registry, banded weighted tables, validation hooks, RON loading | The RON |
| Narration *seam* (`HitView` → `String` behind a trait) | The sentences |
| The balance-checker machinery behind a `ThreatSubject` trait | The threat inputs |

Concretely: replace `enum DamageType { Physical, Fire, Cold, Poison }` with
`DamageTypeId(u16)` + a game-registered `DamageTypeTable { name, badge_label, colour }`. The engine's
resistance ladder, `Resistances(BTreeMap<DamageTypeId, ResistLevel>)`, `ResistAccum` and
`compute_after_armor` all work unchanged. Do the same for statuses, stats, slots, item tags, factions and
tactics. That is six registries and it buys genuine theme-agnosticism, where thirteen closed enums buy none.

**Do ship optional theme crates** (`rogue-theme-fantasy` with the RON and the registry registrations) as
*examples and starting kits*, depending on the engine and never depended on by it. That satisfies "power
fantasy, sci-fi, pirate, Robin Hood" without any of them being in the engine's dependency graph.

### F16. Multi-crate split I would draw (from this half's evidence)

```
rogue-core        Position/grid math, geometry (line, cone, burst, Chebyshev), dice, RunSeed/SeedDomain,
                  RNG streams, interned Id<T>.  No Bevy.
rogue-map         Tiles, FOV, pathfinding (A*, Dijkstra/flow field, HazardMap cost overlay), SpatialGrid,
                  worldgen builder chain.  No Bevy (traits + plain data).   [sibling's scope]
rogue-content     ContentRegistry<T>, BandedWeightedTable<T>, RON loading + validation hooks, Id interning.
                  No Bevy.
rogue-rules       Stat/modifier stack, damage pipeline stages, status registry, hook vocabulary,
                  targeting/AoE resolution, equipment slot graph, turn scheduler.  Plain data + traits;
                  Bevy only behind a feature flag.
rogue-ai          Tactic trait + registry, pure decision helpers, MonsterAI state, ability scorer,
                  auto-explore/travel.  Depends on rogue-map + rogue-rules.  (Optional rogue-ai-goap.)
rogue-bevy        The Bevy plugin layer: components, messages, SystemSets (ProcessingPhase/CombatPhase),
                  the three-owner ordering contract.  This is where CombatPlugin-shaped code lives.
rogue-ui          Theme tokens, list→detail widget, key hints, tab chrome, semantic log.  Bevy UI.
rogue-tools       Balance checker over ThreatSubject, spawn-progression report, seed replay.  No Bevy.
```

The load-bearing rule: `rogue-core`, `rogue-content`, `rogue-rules`, `rogue-ai` and `rogue-tools` must
compile without Bevy. fantasy-rogue proves this is achievable - `src/actors/balance.rs`,
`src/actors/ai/decisions.rs`, `src/actors/ai/targeting.rs`, `src/combat/narrate.rs` and the damage arithmetic
are already Bevy-free and are the highest-value, best-tested code in the whole scope. That is not a
coincidence: the Bevy-free modules are the ones with clean seams, because the type system forced the
dependencies to be explicit.
