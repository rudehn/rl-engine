//! The names a game imports, by glob.
//!
//! [`prelude`](self) is what a first game names, the first three steps of
//! the guide and no more, so a game that globs it beside Bevy's prelude
//! sees a page of names it can read. Every other name a game uses is in
//! exactly one topic module beside it, which a game globs when it takes up
//! that topic: [`combat`], [`items`], [`minds`], [`world`], [`mapgen`],
//! [`ui`], [`save`] and [`extras`]. [`all`] is every one of them, for a
//! game that would rather not choose, or that is moving from a release
//! whose prelude was everything.
//!
//! Each module lists its names rather than globbing a crate's, and
//! `scripts/check-prelude.py` holds them to that: the core to 80 names,
//! no name in two modules, since a game that globs both would find it
//! ambiguous at every use, and no glob but `all`'s. Plumbing a game never
//! names, such as what a subsystem keeps in the air while a cue plays, is
//! in none of them and stays at its crate's path.
//!
//! ```
//! use bevy::prelude::*;
//! use atglyph::prelude::all::*;
//!
//! // One name from each module, named rather than glob-imported, so a
//! // collision with Bevy's prelude or between two modules fails here.
//! fn core(_: Point, _: GridRect, _: Rect, _: Position, _: Health, _: Glyph, _: Chain, _: Seed, _: Log) {}
//! fn topics(_: StatusEvent, _: Inventory, _: Intelligence, _: Turns, _: Prefab, _: Palette, _: Saves, _: Trader) {}
//! // Bevy's own names a game uses beside the engine's: the shape a mesh
//! // is made from, which a grown place's shapes must not shadow.
//! fn bevy_own(_: Rectangle, _: Circle) {}
//! // The three app traits are in the core, so every method resolves.
//! fn build<K: Saveable>(app: &mut App) {
//!     app.add_engine_effects().add_modal("pack").save_kind::<K>();
//! }
//! ```

pub use crate::RoguelikePlugins;
pub use atglyph_bevy::app::EngineApp;
pub use atglyph_bevy::combat::{Armor, CombatPlugin, CombatRules, Faction, Health, MeleeAttack};
pub use atglyph_bevy::components::{Actor, Blocks, MyTurn, Player, Position, RevealsMap, Speed, Viewshed};
pub use atglyph_bevy::knowledge::Knowledge;
pub use atglyph_bevy::lighting::{DarkSight, LightSource, Lighting, LightingPlugin};
pub use atglyph_bevy::minds::{Mind, MindsPlugin, Perception};
pub use atglyph_bevy::places::{MapId, PlaceBuild, PlaceEntered, PlaceRules, SpawnPlayer};
pub use atglyph_bevy::plugin::{EngineSet, NewRun, Turn, TurnSet};
pub use atglyph_bevy::registries::Registries;
pub use atglyph_bevy::seed::Seed;
pub use atglyph_bevy::turn::{Intent, Wait};
pub use atglyph_bevy::world::WorldMap;
pub use atglyph_core::dice::DiceRoll;
pub use atglyph_core::geometry;
pub use atglyph_core::grid::Grid2D;
pub use atglyph_core::point::{GridRect, Point};
pub use atglyph_core::seed::{PlaceSeed, RunSeed, SeedDomain};
pub use atglyph_grid::light::Rgb;
pub use atglyph_grid::tile::{TileLook, TileProps, TileRegistry, TileTables};
pub use atglyph_mapgen::chain::{BuildError, Chain};
pub use atglyph_mapgen::context::BaseContext;
pub use atglyph_mapgen::dungeon;
pub use atglyph_render::map_view::{Glyph, TileAppearance};
pub use atglyph_render::shade::Vary;
pub use atglyph_render::terminal::Cell;
pub use atglyph_rules::ai::Brain;
pub use atglyph_rules::content::Registry;
pub use atglyph_rules::damage::DamageKind;
pub use atglyph_rules::faction::{FactionDef, FactionId};
pub use atglyph_save::run::SaveApp;
pub use atglyph_ui::ViewSet;
pub use atglyph_ui::app::UiApp;
pub use atglyph_ui::controls::ControlInput;
pub use atglyph_ui::facet::Facet;
pub use atglyph_ui::game_menu::GameMenuPanel;
pub use atglyph_ui::log::Log;
pub use atglyph_ui::narrate::{NarratorPlugin, Phrase};
pub use atglyph_ui::panel::{LogPanel, VitalsPanel};
pub use atglyph_ui::player_keys::PlayerKeysPlugin;
pub use atglyph_ui::tone::Tones;
pub use atglyph_ui::view::VitalsView;

/// Blows, shots and what they do: combat and its damage, statuses,
/// abilities, the effects an ability or a trigger lands, throwing, ammo
/// and the bump that turns a step into a blow.
pub mod combat {
    pub use atglyph_bevy::ability::{Abilities, AbilitiesPlugin, AbilityEvent, Cooldowns, Grants, Known, Pools, Use};
    pub use atglyph_bevy::accuracy::{Attempt, HitRules, Missed};
    pub use atglyph_bevy::ammo::{Ammo, AmmoChanged, AmmoPlugin, AmmoSet, Dry};
    pub use atglyph_bevy::bump::{Bump, BumpRules, Bumped, OnAlly, Swapped};
    pub use atglyph_bevy::combat::{
        Attack, AttackShape, BenchedAttack, BonusDamage, DamageDealt, DamageEvent, DamageRoute, DamageStages, Dead, DeathEvent, Heal, Healed, Invulnerable,
        Loadout, RangedAttack, Resists, Struck, line_of_fire, shot,
    };
    pub use atglyph_bevy::effects::{Effect, EffectBonus, EffectKinds, EffectWorld, Effects, EffectsPlugin, Fired, FromArgs, Landing, TriggerKinds, Triggers};
    pub use atglyph_bevy::status::{ActiveStatuses, ApplyStatus, RemoveStatus, StatBlock, StatusEvent};
    pub use atglyph_bevy::throwing::{Throw, Throwable, ThrowingPlugin};
    pub use atglyph_rules::ability::{
        AbilityDef, AbilityId, Aim, Asks, Blocked, Cost, EffectArea, EffectSpec, Gates, Purse, Requirement, Targeting, TriggerSpec, blocked, read_args,
    };
    pub use atglyph_rules::damage::{DamageStage, Hit, Resistances, resolve};
    pub use atglyph_rules::forecast::{Combatant, Duel, Outlook, duel};
    pub use atglyph_rules::stats::{Modifier, ModifierSource, Op, StatDef, StatId, Stats};
    pub use atglyph_rules::status::{ActiveStatus, Mended, SLEEPER_REACH, Stacking, StatusDef, StatusId, Statuses, Tick, TickReport, UNTIL_CURED};
}

/// Things carried: items, equipment, consumables and their charges, loot
/// and its tables, the parts an actor or an item is made from, and tags.
pub mod items {
    pub use atglyph_bevy::consumable::{Attuned, Consumable, WhenEmpty};
    pub use atglyph_bevy::items::{
        Bestows, DropItem, Equip, EquipFromGround, Equipped, GearScore, Inventory, Item, ItemEvent, ItemRules, ItemsPlugin, LitWhenWorn, OwnBlowScore, PickUp,
        Stack, Stuck, Tagged, Unequip, Unknown, UseItem, UseItemAt, UseOn, Wearable, WornStatuses, WornTimer,
    };
    pub use atglyph_bevy::loot::{Drops, Found, ItemMaker, LootArea, LootPlugin, LootSet, Provenance};
    pub use atglyph_bevy::parts::{ActorParts, Attacks, ItemLevel, ItemParts, LightWhen, innate_statuses, insert_parts};
    pub use atglyph_rules::equip::{EquipError, EquipShape, Equipment, SlotDef, SlotId};
    pub use atglyph_rules::loot::{DropRow, DropTable, Layout, LevelRow, LevelTable, LootRow, LootTable, Placed, ScatterRules, plan_scatter};
    pub use atglyph_rules::tag::{TagDef, TagId};
}

/// What the monsters think with: minds and their tactics, factions,
/// stealth and noticing, and noise and hearing.
pub mod minds {
    pub use atglyph_bevy::minds::{Intelligence, JoinStamp, LastTactic, Leader, MindChose, Post, Profile, Thinking};
    pub use atglyph_bevy::noise::{Footfall, Heard, Hearing, MakeNoise, NoiseHeard, NoisePlugin, NoiseRules, SoundId, SoundNames, Sounds};
    pub use atglyph_bevy::stealth::{Aware, Notice, Noticed, Stealth, StealthPlugin, Unseen, Watchers};
    pub use atglyph_rules::ai::{
        ActorView, Awareness, Carried, Choice, Decision, Fields, HearingStats, ItemView, Missile, MovementProfile, NoFields, NoticeStats, Repertoire, Sense,
        Snapshot, Standard, StandardBrain, StealthStats, Tactic, TacticCtx, Vitals, Wits, notices, tactics,
    };
    pub use atglyph_rules::faction::{Factions, Relation};
}

/// The rest of the world a run plays in: lighting beyond the core,
/// fire, gas, props, doors, remains, work, facts and quests, the place
/// model beyond the core, the turn loop's sets and actions, the run's
/// state, the grid's searches and fields, and the primitives under them.
pub mod world {
    pub use atglyph_bevy::doors::{Close, DoorEvent, Open};
    pub use atglyph_bevy::events::{Counters, FactSet, FactsPlugin, Happened, QuestChange, Quests};
    pub use atglyph_bevy::fire::{Burning, Fire, FireEvent, FirePlugin, FireRules, Flammable, Kindle};
    pub use atglyph_bevy::fov::FovPlugin;
    pub use atglyph_bevy::gas::{Breathed, GasPlugin, Gases, Release, Vents};
    pub use atglyph_bevy::lighting::{Concealed, DarkSightSuppressed, Fuel, LightEvent, OwnDarkSight, PlayerSight, WornDarkSight};
    pub use atglyph_bevy::places::{
        Arrive, Climb, Destination, GoThrough, Ladder, LadderEnd, Lock, MapChanged, OnMap, PlaceRulesRes, Spot, Stair, StairLook, Transition, Travelled,
        WarpRequest,
    };
    pub use atglyph_bevy::plugin::{
        CleanupSet, CorePlugin, DecideSet, EndOfFrame, EndRun, FieldSet, LandSet, PerceiveSet, PrepareRun, PresentSet, ReactSet, ResolveSet, RunResumed,
    };
    pub use atglyph_bevy::prefabs::{ActorMaker, PrefabPlugin, PrefabSet, Prefabs};
    pub use atglyph_bevy::props::{
        AsksFirst, Container, Emptied, FillContainer, Finding, Hidden, Interact, Interacted, Offer, OfferChooser, OfferFilter, OfferedHere, Offering, Offers,
        Prop, PropKind, PropSet, PropsPlugin, Refused, Scenery, SpotBonus, SpotRules, Spotted, Take, VerbId, Verbs, spawn_prop,
    };
    pub use atglyph_bevy::remains::{LeavesRemains, Remains, RemainsLeft, RemainsNaming, RemainsPlugin, RevivalRules, ReviveCommands, Revived};
    pub use atglyph_bevy::seed::LaunchArgs;
    pub use atglyph_bevy::state::{Ending, EngineState, Outcome, Restart, RunOver};
    pub use atglyph_bevy::turn::{Acting, Action, ActionDone, ActionRefused, Occupancy, Resolution, Speeds, Step, Stepped, TurnEnd, TurnRules, Turns};
    pub use atglyph_bevy::work::{WorkDone, WorkKinds, WorkPace, Working};
    pub use atglyph_core::direction::Direction;
    pub use atglyph_core::grid::{Grid, Steps};
    pub use atglyph_core::id::{Id, Interner};
    pub use atglyph_core::turn::BASE_ACTION_COST;
    pub use atglyph_grid::astar::{AStar, PathRules};
    pub use atglyph_grid::bitgrid::BitGrid;
    pub use atglyph_grid::dijkstra::DijkstraMap;
    pub use atglyph_grid::field::{Around, TileField};
    pub use atglyph_grid::light::{Emitter, Glow, Light, LightField};
    pub use atglyph_grid::spatial::SpatialGrid;
    pub use atglyph_grid::targeting::{Footprint, TargetMode, burst, clear_shot, footprint};
    pub use atglyph_grid::terrain::{CostSource, OpacitySource, Terrain, TerrainView};
    pub use atglyph_grid::tile::{Burn, Kindling, TileId};
    pub use atglyph_grid::{LightBand, fov};
    pub use atglyph_rules::content::{BandedEntry, BandedTable, ContentError, Named, load_spawns};
    pub use atglyph_rules::events::{
        Change, CounterDef, CounterId, Fact, FactDef, FactKind, Ledger, Matcher, Need, Objective, QuestDef, QuestId, QuestState, Tally, Tracker,
    };
    pub use atglyph_rules::fire::Tinder;
    pub use atglyph_rules::gas::{Breath, GasDef, GasId};
    pub use atglyph_rules::names::{NameRef, Names};
    pub use atglyph_rules::prop::{ContainerDef, ContentRoll, HiddenDef, OfferDef, PropDef, PropId, Stock};
}

/// Growing a place: the map generation passes, pools, prefabs and
/// dressings, and [`grow`](atglyph_rules::grow), which grows one from content files.
pub mod mapgen {
    pub use atglyph_mapgen::accrete::{Accrete, Shape};
    pub use atglyph_mapgen::chain::{Pass, Phase};
    pub use atglyph_mapgen::context::Outputs;
    pub use atglyph_mapgen::dress::{DistrictDressings, Dress, Dressed, Dressing};
    pub use atglyph_mapgen::loops::OpenLoops;
    pub use atglyph_mapgen::passes;
    pub use atglyph_mapgen::pool::{DressAt, Lay, PoolPick, Prefer, StampPicks};
    pub use atglyph_mapgen::prefab::{Orient, Placement, Prefab, PrefabShape, StampEachRoom, StampOneOf, StampPrefab, Stamped};
    pub use atglyph_mapgen::secrets::{HideOpenings, SecretCell};
    pub use atglyph_mapgen::survey::{AreaFacts, Require, Survey, Surveyed};
    pub use atglyph_rules::grow;
    pub use atglyph_rules::prefab::{PrefabDef, PrefabLibrary};
}

/// What is drawn and how a player drives it: panels, views and their
/// plugins, the palette, modals, controls, cursors, the narrator's words,
/// the title, settings, the renderer's plugins and the cues a presenter
/// plays.
pub mod ui {
    #[cfg(not(target_arch = "wasm32"))]
    pub use crate::TtyRoguelikePlugins;
    pub use atglyph_bevy::cue::{Anchor, Cue, CueWait, Cued, LookOf};
    pub use atglyph_bevy::settings::{Setting, SettingId, Settings};
    pub use atglyph_bevy::title::{AbandonRun, ContinueRun, HoldStart, LeaveRun, SavedRun, begin_run};
    pub use atglyph_render::capture::CapturePlugin;
    pub use atglyph_render::fullscreen::FullscreenPlugin;
    pub use atglyph_render::map_view::{DrawnActors, LightOverlay, MapView, MapViewPlugin};
    pub use atglyph_render::particles::{ParticleStyle, Particles, ParticlesPlugin};
    pub use atglyph_render::shade::{Memory, Shading};
    pub use atglyph_render::terminal::{Terminal, TerminalPlugin};
    pub use atglyph_render::tileset::Tileset;
    pub use atglyph_ui::controls::{Chord, ControlId, Controls, ControlsKeys, EngineKey, Keys, RepeatPace, Repeats};
    pub use atglyph_ui::cursor::{CursorKeys, CursorStyle};
    pub use atglyph_ui::focus::{Focus, Sighted, Sighting};
    pub use atglyph_ui::game_menu::{MenuKeys, game_menu_modal};
    pub use atglyph_ui::interact::InteractKeys;
    pub use atglyph_ui::keys::DirectionKeys;
    pub use atglyph_ui::layout::{RailLayout, RailPanels, RailScreen};
    pub use atglyph_ui::log::{LogEntry, MessageLog, Span};
    pub use atglyph_ui::menu::{ListMenu, MenuRow, draw_menu};
    pub use atglyph_ui::modal::{ModalId, Modals, modal_is, modal_open, no_modal};
    pub use atglyph_ui::narrate::{MassNoun, NarrationView, Phrasebook, Said, Tell, Words};
    pub use atglyph_ui::panel::{
        AbilityKeys, AbilityPanel, ActivityWords, AlertWords, Choosing, ChoosingFor, Confirming, ContainerPanel, ControlsPanel, GearPanel, InspectPanel,
        InventoryKeys, InventoryMenu, InventoryPanel, NearbyPanel, OfferChoice, OffersPanel, ScrollbackKeys, ScrollbackPanel, SettingsPanel, TargetPanel,
        TitleMenu, TitlePanel, TitleRow, TitleSet, TitleWords, ability_modal, container_modal, controls_modal, inventory_modal, offers_modal, settings_modal,
        title_is_up,
    };
    pub use atglyph_ui::player_keys::{Declined, Otherwise, PlayerKeysSet, PlayerVerb};
    pub use atglyph_ui::replay::ReplayPlugin;
    pub use atglyph_ui::tone::{Palette, ToneId, readable};
    pub use atglyph_ui::view::{
        AbilityRow, AbilityView, AbilityViewPlugin, AimAt, AimFire, AimRange, AimThrow, AimUse, Asking, Bar, ContainerView, ContainerViewPlugin, EndingSection,
        EndingView, GearView, GearViewPlugin, InspectView, InspectViewPlugin, InventoryView, InventoryViewPlugin, ItemRow, NearbyFollowers, NearbyStatuses,
        NearbyView, NearbyViewPlugin, OfferRow, OffersView, OffersViewPlugin, OpenContainer, Row, Seems, TargetView, TargetViewPlugin, VitalsViewPlugin,
        WorkRow, target_modal,
    };
    pub use atglyph_ui::{UiPlugin, panel};
}

/// Saving a run: [`SavePlugin`](atglyph_save::SavePlugin), what a kind
/// says about itself to be saved, and the backends.
pub mod save {
    pub use atglyph_save::backend::{FileBackend, MemoryBackend, SaveBackend, SaveError, Saves};
    pub use atglyph_save::engine::EngineSave;
    pub use atglyph_save::remap::SaveId;
    pub use atglyph_save::run::{RunSave, SavePlugin, Saveable, SaveableActor, SaveableItem, SaveableState, load_run, save_run};
    pub use atglyph_save::unload::Stash;
}

/// What one game in the repository uses and a first game seldom
/// does: trading, run records, the party and sheet panels, lakes, name
/// generation, threat scoring and reflection.
pub mod extras {
    pub use atglyph_bevy::ability::{Reflected, Reflects};
    pub use atglyph_bevy::records::{RecordOutcome, RecordRun, RecordSection, RecordsPlugin, RunRecord, RunRecords};
    pub use atglyph_bevy::trade::{Buy, Currency, Deal, Parties, Sell, TRADE, TradePlugin, TradeRefusal, TradeRefused, Traded, Trader, Trading, Ware, Worth};
    pub use atglyph_mapgen::lakes::{Bridges, Lakes};
    pub use atglyph_rules::balance::{BandRow, Report, ThreatSubject, threat};
    pub use atglyph_rules::namegen::{NameBook, NameSet};
    pub use atglyph_ui::panel::{PartyPanel, RecordsPanel, SheetKeys, SheetPanel, TradeKeys, TradePanel, TradeWords, records_modal, sheet_modal, trade_modal};
    pub use atglyph_ui::view::{PartyView, PartyViewPlugin, SheetView, SheetViewPlugin, TradeRow, TradeView, TradeViewPlugin};
}

/// Every module above, for a game that would rather glob one line than
/// choose.
pub mod all {
    pub use super::combat::*;
    pub use super::extras::*;
    pub use super::items::*;
    pub use super::mapgen::*;
    pub use super::minds::*;
    pub use super::save::*;
    pub use super::ui::*;
    pub use super::world::*;
    pub use super::*;
}
