//! The ledger: tasks from RON, the facts that advance them, and the run's
//! victory.
//!
//! The engine tracks quests as objectives over facts. This module names
//! the facts Corsair reports, turns the game's outcomes into them, loads
//! the tasks from RON with names resolved to ids, narrates what moves,
//! ends the run when the winning task is done, and draws the ledger.

use std::collections::BTreeMap;

use bevy::prelude::*;
use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_core::{Point, Rect};
use rl_engine::rl_render::Terminal;
use rl_engine::rl_rules::{Fact, FactDef, FactKind, Matcher, Need, Objective, QuestDef, QuestState};
use rl_engine::rl_rules::{Named, Registry};
use rl_engine::rl_ui::{ListMenu, MenuRow, MessageLog, Modals, Palette, Tones, draw_menu};

use crate::content::{COVE, PORT};
use crate::items::{Armory, ItemKind};
use crate::monsters::{Bestiary, MonsterKind};
use crate::places;

const QUESTS_RON: &str = include_str!("../assets/quests.ron");

/// The kinds of fact Corsair reports.
#[derive(Resource)]
pub struct Facts {
    pub killed: FactKind,
    pub killed_faction: FactKind,
    pub picked_up: FactKind,
    pub carrying: FactKind,
    pub used: FactKind,
    pub equipped: FactKind,
    pub entered_cave: FactKind,
    pub entered_site: FactKind,
    defs: Registry<FactDef>,
}

impl Facts {
    fn new() -> Self {
        let defs = Registry::from_defs(
            ["killed", "killed_faction", "picked_up", "carrying", "used", "equipped", "entered_cave", "entered_site"].map(FactDef::new).to_vec(),
        )
        .unwrap();
        Self {
            killed: defs.expect("killed"),
            killed_faction: defs.expect("killed_faction"),
            picked_up: defs.expect("picked_up"),
            carrying: defs.expect("carrying"),
            used: defs.expect("used"),
            equipped: defs.expect("equipped"),
            entered_cave: defs.expect("entered_cave"),
            entered_site: defs.expect("entered_site"),
            defs,
        }
    }
}

/// A task as authored.
#[derive(Debug, Clone, serde::Deserialize)]
struct QuestRon {
    name: String,
    title: String,
    text: String,
    #[serde(default)]
    after: Vec<String>,
    objectives: Vec<ObjectiveRon>,
    #[serde(default)]
    victory: bool,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct ObjectiveRon {
    text: String,
    on: MatcherRon,
    need: Need,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct MatcherRon {
    kind: String,
    #[serde(default)]
    subject: Option<String>,
}

impl Named for QuestRon {
    fn name(&self) -> &str {
        &self.name
    }
}

/// Loads the tasks, resolving every name to an id; panics listing every
/// problem, as the other content loaders do.
pub fn load(bestiary: &Bestiary, armory: &Armory, registries: &Registries) -> (Quests, Facts) {
    let facts = Facts::new();
    let authored: Registry<QuestRon> = Registry::from_ron_str(QUESTS_RON).unwrap_or_else(|e| panic!("assets/quests.ron: {e}"));
    let subject = |kind: &str, name: &str| -> Result<u64, String> {
        let unknown = || format!("unknown {kind} subject {name:?}");
        match kind {
            "killed" => bestiary.defs.id(name).map(|id| id.raw() as u64).ok_or_else(unknown),
            "killed_faction" => registries.factions.id(name).map(|id| id.raw() as u64).ok_or_else(unknown),
            "picked_up" | "carrying" | "used" | "equipped" => armory.defs.id(name).map(|id| id.raw() as u64).ok_or_else(unknown),
            "entered_cave" => name.parse::<u64>().map_err(|_| unknown()),
            "entered_site" => match name {
                "port" => Ok(PORT.0 as u64),
                "cove" => Ok(COVE.0 as u64),
                _ => Err(unknown()),
            },
            _ => Err(format!("unknown fact kind {kind:?}")),
        }
    };
    authored
        .validate(|q, all| {
            for a in &q.after {
                if all.id(a).is_none() {
                    return Err(format!("{}: after unknown task {a:?}", q.name));
                }
            }
            if q.objectives.is_empty() {
                return Err(format!("{}: no objectives", q.name));
            }
            for o in &q.objectives {
                if facts.defs.id(&o.on.kind).is_none() {
                    return Err(format!("{}: unknown fact kind {:?}", q.name, o.on.kind));
                }
                if let Some(s) = &o.on.subject {
                    subject(&o.on.kind, s).map_err(|e| format!("{}: {e}", q.name))?;
                }
            }
            Ok(())
        })
        .unwrap_or_else(|e| panic!("assets/quests.ron: {e}"));
    let defs: Vec<QuestDef> = authored
        .iter()
        .map(|(_, q)| QuestDef {
            name: q.name.clone(),
            title: q.title.clone(),
            text: q.text.clone(),
            after: q.after.iter().map(|a| rl_engine::rl_core::Id::from_raw(authored.expect(a).raw())).collect(),
            objectives: q
                .objectives
                .iter()
                .map(|o| {
                    let mut on = Matcher::any(facts.defs.expect(&o.on.kind));
                    if let Some(s) = &o.on.subject {
                        on = on.about(subject(&o.on.kind, s).expect("validated"));
                    }
                    Objective { text: o.text.clone(), on, need: o.need }
                })
                .collect(),
            victory: q.victory,
        })
        .collect();
    (Quests::new(Registry::from_defs(defs).unwrap()), facts)
}

/// What fact reporting reads.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Outcomes<'w, 's> {
    deaths: MessageReader<'w, 's, DeathEvent>,
    items: MessageReader<'w, 's, ItemEvent>,
    entered: MessageReader<'w, 's, PlaceEntered>,
    facts: Res<'w, Facts>,
    world: Res<'w, WorldRes>,
    map: Res<'w, WorldMap>,
    monsters: Query<'w, 's, (&'static MonsterKind, &'static Faction)>,
    kinds: Query<'w, 's, (&'static ItemKind, Option<&'static Stack>)>,
    player: Query<'w, 's, (&'static Position, &'static Inventory), With<Player>>,
}

/// Turns the frame's outcomes into facts.
pub fn report_facts(mut out: Outcomes, mut happened: MessageWriter<Happened>, mut last_region: Local<Option<Point>>) {
    let facts = &out.facts;
    let mut report = |f: Fact| happened.write(Happened(f));
    for d in out.deaths.read() {
        if let Ok((kind, faction)) = out.monsters.get(d.entity) {
            report(Fact::new(facts.killed).about(kind.0.raw() as u64));
            report(Fact::new(facts.killed_faction).about(faction.0.raw() as u64));
        }
    }
    let carried: BTreeMap<u64, i64> = out
        .player
        .single()
        .map(|(_, bag)| {
            let mut totals = BTreeMap::new();
            for (kind, stack) in bag.items.iter().filter_map(|i| out.kinds.get(*i).ok()) {
                *totals.entry(kind.0.raw() as u64).or_insert(0) += stack.map(|s| s.count as i64).unwrap_or(1);
            }
            totals
        })
        .unwrap_or_default();
    for ev in out.items.read() {
        let (kind, item) = match *ev {
            ItemEvent::PickedUp { item, merged_into, .. } => (facts.picked_up, merged_into.unwrap_or(item)),
            ItemEvent::Used { item, .. } => (facts.used, item),
            ItemEvent::Equipped { item, .. } => (facts.equipped, item),
            _ => continue,
        };
        let Ok((def, _)) = out.kinds.get(item) else { continue };
        let def = def.0.raw() as u64;
        report(Fact::new(kind).about(def));
        if kind == facts.picked_up {
            report(Fact::new(facts.carrying).about(def).amount(carried.get(&def).copied().unwrap_or(0)));
        }
    }
    for ev in out.entered.read() {
        if let Some((_, depth)) = places::cave_of(ev.map) {
            report(Fact::new(facts.entered_cave).about(depth as u64 + 1));
        }
    }
    // Arriving in a site's region on the surface.
    if out.map.current().is_surface()
        && let Ok((pos, _)) = out.player.single()
    {
        let region = out.world.region_of_tile(pos.0);
        if *last_region != Some(region) {
            *last_region = Some(region);
            if let Some(site) = out.world.site_at(region) {
                report(Fact::new(facts.entered_site).about(site.kind.0 as u64));
            }
        }
    } else {
        *last_region = None;
    }
}

/// Narrates what the tracker reported, and ends the run on victory.
pub fn narrate_quests(
    mut changes: MessageReader<QuestChange>,
    quests: Res<Quests>,
    turns: Res<Turns>,
    mut log: ResMut<MessageLog>,
    mut next: ResMut<NextState<EngineState>>,
) {
    let turn = turns.turn_number();
    for c in changes.read() {
        match c.0 {
            rl_engine::rl_rules::Change::Progress { .. } => {}
            rl_engine::rl_rules::Change::ObjectiveDone { quest, objective } => {
                log.push(format!("{}: done.", quests.defs.get(quest).objectives[objective].text), Tones::GOOD, turn);
            }
            rl_engine::rl_rules::Change::QuestDone { quest, victory } => {
                let q = quests.defs.get(quest);
                log.push(format!("Task complete: {}.", q.title), Tones::NOTICE, turn);
                if victory {
                    log.push("You have won. The sea is yours. Press q to quit.", Tones::NOTICE, turn);
                    next.set(EngineState::Idle);
                }
            }
            rl_engine::rl_rules::Change::QuestOpened { quest } => {
                log.push(format!("New task: {}. [t]", quests.defs.get(quest).title), Tones::NOTICE, turn);
            }
        }
    }
}

/// The ledger screen.
#[derive(Resource)]
pub struct LedgerScreen {
    pub menu: ListMenu,
}

impl Default for LedgerScreen {
    fn default() -> Self {
        let mut menu = ListMenu::new("Ledger");
        menu.hints = "[esc]".into();
        menu.empty = "No tasks yet.".into();
        Self { menu }
    }
}

/// Opens, closes and scrolls the ledger.
/// The name the ledger's modal is declared under.
pub const MODAL: &str = "ledger";

/// The id of the ledger's modal.
pub fn modal(modals: &Modals) -> rl_engine::rl_ui::ModalId {
    modals.get(MODAL).expect("main declares the ledger modal")
}

pub fn ledger_keys(keys: Res<ButtonInput<KeyCode>>, mut screen: ResMut<LedgerScreen>, mut modals: ResMut<Modals>) {
    let ledger = modal(&modals);
    if keys.just_pressed(KeyCode::KeyT) && (modals.is_top(ledger) || !modals.any_open()) {
        modals.toggle(ledger);
        return;
    }
    if !modals.is_top(ledger) {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        modals.close_one(ledger);
    }
    if keys.any_just_pressed([KeyCode::ArrowDown, KeyCode::KeyJ]) {
        screen.menu.move_by(1);
    }
    if keys.any_just_pressed([KeyCode::ArrowUp, KeyCode::KeyK]) {
        screen.menu.move_by(-1);
    }
}

/// Fills the ledger from the tracker and draws it.
pub fn draw_ledger(mut screen: ResMut<LedgerScreen>, modals: Res<Modals>, quests: Res<Quests>, palette: Res<Palette>, mut terminal: ResMut<Terminal>) {
    if !modals.is_open(modal(&modals)) {
        return;
    }
    let mut rows = Vec::new();
    for state in [QuestState::Open, QuestState::Done] {
        for quest in quests.tracker.in_state(state) {
            let q = quests.defs.get(quest);
            let steps: Vec<String> = q
                .objectives
                .iter()
                .enumerate()
                .map(|(i, o)| {
                    let (have, need) = quests.tracker.progress(quest, i, &quests.defs);
                    if have >= need { format!("{} (done)", o.text) } else { format!("{} ({have}/{need})", o.text) }
                })
                .collect();
            let done = state == QuestState::Done;
            rows.push(MenuRow::new(q.title.clone()).tag(if done { "done" } else { "" }).detail(format!("{} {}", q.text, steps.join("; "))).toned(if done {
                Tones::MUTED
            } else {
                Tones::TEXT
            }));
        }
    }
    screen.menu.set_rows(rows);
    let bounds = terminal.bounds();
    let wanted = screen.menu.rows.len().max(3) as i32 + 5;
    let (w, h) = (bounds.width.min(70), bounds.height.min(22).min(wanted));
    let rect = Rect::new((bounds.width - w) / 2, (bounds.height - h) / 2, w, h);
    draw_menu(&mut terminal, rect, &screen.menu, &palette);
}

#[cfg(test)]
mod tests {
    use super::*;
    use rl_engine::rl_core::RunSeed;

    #[test]
    fn the_tasks_load_and_chain_to_a_victory() {
        let loaded = crate::rules::load(RunSeed(1), Point::ZERO, &crate::rules::effect_kinds());
        let (armory, facts) = (&loaded.armory, &loaded.facts);
        let quests = &loaded.quests;
        let retire = quests.defs.expect("retire");
        let hoard = quests.defs.expect("hoard");
        assert!(quests.defs.get(retire).victory);
        assert_eq!(quests.defs.get(retire).after, vec![hoard]);
        assert_eq!(quests.tracker.state(quests.defs.expect("sea_legs")), QuestState::Open);
        assert_eq!(quests.tracker.state(hoard), QuestState::Locked);
        let carry = &quests.defs.get(hoard).objectives[1];
        assert_eq!(carry.on, Matcher::any(facts.carrying).about(armory.defs.expect("doubloons").raw() as u64));
        assert_eq!(carry.need, Need::Latest(100));
    }
}
