## Conflict Detection Report

Ingest mode: new.
Docs: 24 (1 ADR, 4 SPEC, 0 PRD, 19 DOC).
Precedence: ADR > SPEC > PRD > DOC; no per-doc overrides.
Locked: docs/PLAN.md only.
Inside docs/PLAN.md, later progress-log entries supersede section 3 text (intra-document evolution, INFO).

### BLOCKERS (0)

None.
Checks run: LOCKED-vs-LOCKED ADR contradictions (only one locked ADR is in the set, so none are possible); UNKNOWN or low-confidence classifications (none; the lowest confidence is medium); traversal depth cap (24 nodes, under 50).
The cross-ref cycle check found cycles; see the first WARNING for why they are not BLOCKERS.

### WARNINGS (2)

[WARNING] Cross-reference cycles found, downgraded from BLOCKER after review
  Found: One strongly connected component of 17 docs in the cross_refs graph (three-color DFS plus Tarjan).
    Members: docs/PLAN.md, docs/OVERVIEW.md, docs/TODO.md, docs/design/abilities.md, docs/design/fields.md, docs/design/minds.md, docs/design/stealth.md, docs/design/ui.md, and docs/guide/src/01 through 09.
    Example back edges: docs/PLAN.md -> docs/design/ui.md -> docs/PLAN.md; docs/PLAN.md -> docs/TODO.md -> docs/PLAN.md; docs/design/minds.md -> docs/TODO.md -> docs/design/minds.md; docs/guide/src/01-a-map-and-walking.md -> 03-blows-and-the-log.md -> 01-a-map-and-walking.md; docs/guide/src/09-where-to-go-next.md -> itself.
    One edge was left out: docs/reviews/fantasy_rogue_core.md cites "TODO.md", but that is the reviewed fantasy-rogue repo's file, not docs/TODO.md (source: that doc's classification note).
  Impact: The synthesizer contract says cycles are unresolved BLOCKERs and the cyclic set is not synthesized.
    Followed literally, 17 of 24 docs, including the only ADR, would be excluded and the ingest blocked.
    These edges are "see also" hyperlinks and citations between a plan, its design docs, an inventory, a backlog and tutorial chapters.
    They are not content derived from each other.
    Synthesis here reads each doc once, applies precedence per claim, and never follows a reference recursively, so the loop the rule guards against cannot happen.
    The cyclic set was synthesized.
  → Confirm that mutual hyperlinks between these docs are acceptable and the synthesis stands. Otherwise re-run with a --manifest that excludes the guide chapters (removes 9 of the 17) and accept that PLAN, OVERVIEW, TODO and the design docs still link to each other.

[WARNING] Deferred scope with no owner: promised by locked PLAN sections or SPEC phases, but absent from every current work list
  Found: These items were promised or deferred in /Users/nathanrude/Development/rl-engine/docs/PLAN.md (locked) or in a SPEC phase.
    None of them appears in the PLAN "Next" line, /Users/nathanrude/Development/rl-engine/docs/TODO.md, or the "Not built yet" list in /Users/nathanrude/Development/rl-engine/docs/OVERVIEW.md.
    - M7 Corsair sea scope: ships as vehicles, boarding deck maps, navy, pirate and merchant factions, a treasure-map dig quest, weather as a TileField (source: docs/PLAN.md section 8 M7).
    - M4 world-generation deferrals: river channels and bridges inside chunks beyond the channel pass, fords, cullers beyond keep-largest, a choke map, decoration rules, richer settlements (source: docs/PLAN.md section 3.4, M4, progress 2026-09-10); sconces as prefab marks (source: docs/design/lighting.md phase E).
    - A generated victory condition and generated encounters (source: docs/PLAN.md M5, progress 2026-09-10).
    - Auto-explore and travel-to-a-seen-tile with an ExploreInterrupt trait (source: docs/PLAN.md sections 3.5.2, 3.7, 5); no implementation found in crates/.
    - Stealth phase E sneak attacks, two-way stealth, noise (source: docs/design/stealth.md phase E and section 9; docs/PLAN.md progress 2026-09-12).
    - A lighting shadow layer for negative emitters (source: docs/design/lighting.md phase F).
    - A Summon engine effect (source: docs/design/abilities.md sections 3.5 and 8).
    - Benches for the choke map, each builder, the world chain at three sizes, and the render sweep (source: docs/PLAN.md section 3.9); only fov, astar, dijkstra, regions, light and tile_field exist in crates/rl-grid/benches/grid.rs.
    - Speed through a SpeedModifier trait or ActionCost message (source: docs/PLAN.md section 3.6); neither name found in crates/.
  Impact: Many of these milestones were defined around WildReach, which was abandoned 2026-09-10.
    The docs never say whether the rest was dropped or only paused.
    If the roadmapper includes them, the roadmap may fill with work nobody wants.
    If it leaves them out, locked promises could be silently lost.
  → Mark each group as in scope, later, or dropped before routing. They are staged in /Users/nathanrude/Development/rl-engine/.planning/intel/requirements.md section C as REQ-UNSCOPED-* for that choice. Any group marked dropped should get a PLAN progress-log entry saying so.

### INFO (18)

[INFO] Auto-resolved: PLAN progress log > PLAN section 4 on crate layout
  Note: docs/PLAN.md section 4 lists rl-content, rl-events, rl-ai, rl-tools and rl-test-support as crates. docs/PLAN.md progress 2026-09-11 makes the first four modules of rl-rules (content, events, ai, balance) and deletes rl-test-support; section 4 itself carries a "Revised 2026-09-11" line. The later entry wins. The effective layout is recorded in decisions.md ADR-PLAN-4.

[INFO] Auto-resolved: PLAN progress log > PLAN section 3.11 on Bevy-layer mechanisms
  Note: docs/PLAN.md section 3.11 names a TestApp builder, ProcessingPhase and CombatPhase sets, and a MapBound marker, and section 3.12 references EnginePlugins. Progress entries replace them: TurnSet, DecideSet, ResolveSet and CleanupSet (2026-09-11 to 2026-09-15); opt-in plugins with app.needs (2026-09-11, 2026-09-13); EnginePlugins removed and RoguelikePlugins added (2026-09-13); rl_bevy::testing (2026-09-13); OnMap and places. The later entries win. The principle, named phases with no ordering on another crate's function, is unchanged.

[INFO] Auto-resolved: PLAN section 3.12 in-section revisions > original 3.12 text on the UI backend
  Note: docs/PLAN.md section 3.12 first said Bevy UI and a GlyphGrid drawn as one instanced mesh. Its own revisions (2026-09-11, 2026-09-12) replace that with glyph-terminal presenters over backend-agnostic views, and defer Bevy UI presenters until a game asks. Instanced rendering was never built. docs/OVERVIEW.md lists it as not built, with one sprite per cell as the known scaling limit, while docs/PLAN.md sections 2 and 6 still lock out one entity per cell. It is carried as REQ-instanced-terminal-rendering.

[INFO] Auto-resolved: PLAN progress log > PLAN section 3.10 on rules-layer seams
  Note: docs/PLAN.md section 3.10 names a Narrator trait, a ReactionQueue, AttackIntent, ContentRegistry<T> and BandedWeightedTable. Built instead: NarratorPlugin with a Phrasebook (progress 2026-09-15), TurnSet::React (2026-09-11), Intent<Attack> as an action type (2026-09-11), and Registry<T> and BandedTable (docs/OVERVIEW.md). The later entries win. The shape-in-engine, vocabulary-in-game split is unchanged.

[INFO] Auto-resolved: PLAN progress log > PLAN owner pre-decisions on the first consumer
  Note: docs/PLAN.md "Decisions Nate has already made" and section 8 define milestones as WildReach slices, with a 4096x4096 world as the design point. Progress 2026-09-10 abandons WildReach and makes Corsair the consumer. Progress 2026-09-13 consolidates to one open world (Corsair) and one dungeon (delve). docs/OVERVIEW.md also lists examples/heist, which no progress entry records. The later entries win. The unowned WildReach-era scope is surfaced in the WARNING above.

[INFO] Auto-resolved: PLAN "Next" line > PLAN sections 3.5.1 and 3.5.2 on overworld travel (flag for roadmap scoping)
  Note: docs/PLAN.md "Decisions Nate has already made", 3.5.1 and 3.5.2 lock these points: travel does not simulate time; there is no route-travel command; the overworld never owns the player's position; lwr's Scale and Travel state machine is not ported. The "Next" line (Nate, 2026-09-10, later than the 2026-09-09 revision) says the living-world-rogue conversion keeps overworld token movement, so rl-overworld regains travel on the map. As instructed, the later entry wins inside the ADR. The conversion phase should say which overworld invariants still hold, since this reverses a named owner decision (requirements.md REQ-lwr-conversion).

[INFO] Auto-resolved: PLAN progress log and code > docs/design/ui.md superseded sections
  Note: docs/design/ui.md is a SPEC whose section 11b supersedes its earlier text: Name not Label; views in tier 2; collectors every frame; unset tones reported at startup (section 12 still says "warns once"); ChromeLayout removed with no replacement. Other stale text is resolved by the ADR and the code. Section 2 puts collectors in PresentSet::Narrate, but docs/PLAN.md progress 2026-09-12 uses ViewSet::Annotate, crates/rl-ui/src/lib.rs:139-147 defines ViewSet::{Collect, Annotate, Speak}, and docs/guide/src/09-where-to-go-next.md and docs/OVERVIEW.md say ViewSet::Collect. Section 6 and phase E say EquipView, but the progress log says GearView. Section 0 says "EnginePlugins gains nothing", but EnginePlugins was removed in 2026-09-13. Lamplight is referenced, but it was folded into delve on 2026-09-13. The effective contract is in constraints.md CON-ui-*, and the doc fix is REQ-doc-drift-design-docs.

[INFO] Auto-resolved: PLAN progress log > docs/design/abilities.md on the engine effect set
  Note: docs/design/abilities.md is a SPEC. Its RON header lists effects as Harm, Mend, Afflict, Cleanse, Shove, Pull, Teleport, Kindle, Summon and Grant. Its section 2 table uses Afflict and Kindle(dark), and section 8 lists Shove and Pull as not yet landed. Its own preamble (Inflict, not Afflict; Grant became a rebuilt Known), docs/PLAN.md progress 2026-09-12 and 2026-09-13 (seven effects: Harm, Mend, Inflict, Cleanse, Shove, Pull, Teleport) and progress 2026-09-15 (Ignite and Emit registered by the fire and gas plugins; Kindle is a fire request) give the as-built set. The ADR wins. Summon is unbuilt and sits in the WARNING above.

[INFO] Auto-resolved: PLAN progress log and code > lighting.md (SPEC), OVERVIEW.md and guide chapter 9 (DOC) on how lighting is turned on
  Note: docs/design/lighting.md section 3 ("Inserting it turns lighting on"), docs/OVERVIEW.md ("Lighting, opt-in by inserting `Lighting`") and docs/guide/src/09-where-to-go-next.md ("Add one resource") all say inserting the Lighting resource enables lighting. docs/PLAN.md progress 2026-09-11 lists LightingPlugin as added by name. crates/rl-bevy/src/lighting.rs:306-315 shows LightingPlugin inserting Lighting::dark() itself. Chapter 3, examples/heist and templates/starter all add the plugin. The ADR wins over both lower-precedence sources. Also stale in lighting.md: section 3 describes change-detection dirtying, but its phase B note says recasting happens when sorted emitter lists differ; and Fuel ticks in ResolveSet::Effects (code), not on TurnEnd. Doc fix: REQ-doc-drift-lighting-opt-in.

[INFO] Auto-resolved: as-built record > proposed SPEC phase on how burning tiles glow
  Note: Proposed phase F in docs/design/lighting.md says burning tiles inject into the static layer by flood. docs/design/fields.md (DOC, as built) says burning cells glow through Lighting::set_glow and are cast in the dynamic layer, and crates/rl-bevy/src/lighting.rs:155-162 agrees. docs/PLAN.md progress 2026-09-15 (ADR) records the glow as built. A proposed phase is not a binding contract, so the as-built behaviour recorded in the ADR stands. The rest of phase F, a shadow layer for negative emitters, is listed in the WARNING above.

[INFO] Auto-resolved: docs/design/stealth.md section 12b and PLAN progress 2026-09-16 > stealth.md earlier sections
  Note: docs/design/stealth.md sections 0 and 3 describe the player's viewshed as the one line-of-sight oracle and perceivable as the check. Section 4 passes forget_after as an argument, and section 11 has notices take perception. Section 12b (NoticeStats and StealthStats as tier-1 names, memory on NoticeStats, notices without perception, StealthRunning, one viewshed grid) and docs/PLAN.md progress 2026-09-16 (every actor carries its own Viewshed) supersede them. The effective contract is in constraints.md CON-stealth-*.

[INFO] Recorded: owner decisions in docs/design/minds.md (DOC) treated as locked where the ADR corroborates them
  Note: docs/design/minds.md records "Decision, Nate, 2026-09-16" twice: actors carry their own Viewshed, and Health is {current, max}. The first is repeated in docs/PLAN.md progress 2026-09-16. The second agrees with docs/PLAN.md progress 2026-09-15 ("health stays a component"). No contradiction. Both are recorded in decisions.md as DEC-MINDS-01 and DEC-MINDS-02. The design docs were typed inconsistently (fields.md and minds.md as DOC; lighting, stealth, ui and abilities as SPEC). The firm contracts in fields.md and minds.md were checked against the ADR and found consistent, so the type difference changed no outcome.

[INFO] Auto-resolved: PLAN > docs/TODO.md on the changelog
  Note: docs/TODO.md section 6 says "there is no `CHANGELOG.md`". docs/PLAN.md progress 2026-09-14 says CHANGELOG.md records 0.1.0, and a changelog exists at the repo root with a 0.2.0 section. The ADR wins, and the TODO statement is stale. docs/TODO.md also says it was written 2026-09-15 but counts "commits in the 30 days to 2026-09-17", an inconsistent date.

[INFO] Recorded: open decision on the crate family name does not contradict the locked repo name
  Note: docs/TODO.md section 6 says rl-core is taken on crates.io, so the rl- crate family needs a new name before the first crates.io release. docs/PLAN.md (locked) decides "The engine repo is rl-engine", which names the repository, not crates.io packages. Its releases so far are git tags (progress 2026-09-14). The scopes differ, so nothing needs resolving. The owner decision is still open and is carried as REQ-crate-family-name.

[INFO] Auto-resolved: PLAN > review recommendations it chose not to follow
  Note: The reviews are evidence, and the ADR decided against some of their advice. docs/reviews/living_world_rogue.md F3 suggests generic parameters with trait bounds, which docs/PLAN.md section 3.2 rejects (no generic Map<T: TileSemantics>). living_world_rogue.md F3 and docs/reviews/roguelike_engine.md F4 suggest shipping a worked theme or default content crate; section 3.1 keeps shared content in examples and the full theme in a game. living_world_rogue.md F9 suggests change of scale as a first-class concept; section 3.5 keeps the seam hash and chunks but drops the mode switch. docs/reviews/fantasy_rogue_game.md F7 suggests a Narrator trait; progress 2026-09-15 built a Phrasebook. The ADR wins in each case.

[INFO] Auto-resolved: current guide files > PLAN progress 2026-09-12 on guide chapter numbering
  Note: docs/PLAN.md progress 2026-09-12 says the guide gains 10-panels.md, with testing and where-to-go-next renumbered to 11 and 12. docs/guide/src/SUMMARY.md now has nine chapters, with panels and testing folded into 09-where-to-go-next.md, and docs/guide/src/introduction.md describes six playable steps. The progress entry is history. The later restructure (commits 3ff1c58 and 5eff35a) and examples/heist have no progress-log entry. The current files are taken as current. Doc fix: REQ-doc-drift-overview-and-guide.

[INFO] Noted: stale stamp and self-reference in DOC sources
  Note: docs/OVERVIEW.md says "Last updated: 2026-09-15, after the narrator, the menu and the morgue", but it describes 2026-09-16 work (own viewsheds, the perceive stage, Watchers through viewsheds). The "The crates" section of docs/guide/src/09-where-to-go-next.md says "the tests in chapter 9" from inside chapter 9. Neither conflicts with a decision. Doc fix: REQ-doc-drift-overview-and-guide.

[INFO] Noted: no PRDs in the ingest set
  Note: None of the 24 classifications is a PRD, so no requirement has PRD acceptance criteria and there are no competing acceptance variants. requirements.md is built from the ADR "Next" line, unmet locked decisions, proposed SPEC phases, docs/TODO.md and docs/OVERVIEW.md "Not built yet", and each entry says which. The roadmapper should set acceptance criteria per phase.
