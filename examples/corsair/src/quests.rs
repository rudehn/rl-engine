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
use rl_engine::rl_rules::faction::FactionDef;
use rl_engine::rl_rules::{ContentError, Fact, FactDef, FactKind, Matcher, NameRef, Names, Need, Objective, QuestDef, QuestState};
use rl_engine::rl_rules::{Named, Registry};
use rl_engine::rl_ui::{ControlInput, ListMenu, MenuRow, MessageLog, Modals, Palette, Tones, draw_menu};

use crate::content::{COVE, PORT};
use crate::input::Binds;
use crate::items::{Armory, ItemDef, ItemKind};
use crate::monsters::{Bestiary, MonsterDef, MonsterKind};
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
    on: On,
    need: Need,
}

/// What an objective counts, and of what; `None` counts any.
///
/// One variant per fact Corsair reports, each naming its subject in the
/// words that fact is about: a monster, a side, an item, a cave level or a
/// kind of site. So a subject that names the wrong kind of thing is a parse
/// error, and a name nothing registered is reported by the load.
#[derive(Debug, Clone, serde::Deserialize)]
enum On {
    Killed(Option<NameRef<MonsterDef>>),
    KilledFaction(Option<NameRef<FactionDef>>),
    PickedUp(Option<NameRef<ItemDef>>),
    Carrying(Option<NameRef<ItemDef>>),
    Used(Option<NameRef<ItemDef>>),
    Equipped(Option<NameRef<ItemDef>>),
    EnteredCave(Option<u64>),
    EnteredSite(Option<Site>),
}

/// The kinds of site a task can send the player to.
#[derive(Debug, Clone, Copy, serde::Deserialize)]
enum Site {
    Port,
    Cove,
}

impl On {
    /// The matcher the tracker counts facts with.
    fn matcher(&self, facts: &Facts) -> Matcher {
        let id = |r: &Option<NameRef<_>>| r.map(|r: NameRef<_>| u64::from(r.id().raw()));
        let (kind, subject) = match self {
            On::Killed(m) => (facts.killed, m.map(|r| u64::from(r.id().raw()))),
            On::KilledFaction(f) => (facts.killed_faction, f.map(|r| u64::from(r.id().raw()))),
            On::PickedUp(i) => (facts.picked_up, id(i)),
            On::Carrying(i) => (facts.carrying, id(i)),
            On::Used(i) => (facts.used, id(i)),
            On::Equipped(i) => (facts.equipped, id(i)),
            On::EnteredCave(level) => (facts.entered_cave, *level),
            On::EnteredSite(site) => (
                facts.entered_site,
                site.map(|s| {
                    u64::from(match s {
                        Site::Port => PORT.0,
                        Site::Cove => COVE.0,
                    })
                }),
            ),
        };
        let any = Matcher::any(kind);
        match subject {
            Some(s) => any.about(s),
            None => any,
        }
    }
}

impl Named for QuestRon {
    fn name(&self) -> &str {
        &self.name
    }
}

/// Loads the tasks against the monsters, items and sides they name; panics
/// listing every problem, as the other content loaders do.
pub fn load(bestiary: &Bestiary, armory: &Armory, registries: &Registries) -> (Quests, Facts) {
    let facts = Facts::new();
    let names = registries.names().with("monster", &bestiary.defs).with("item", &armory.defs);
    let defs = tasks(QUESTS_RON, &names, &facts).unwrap_or_else(|e| panic!("assets/quests.ron: {e}"));
    (Quests::new(defs), facts)
}

/// The tasks in `text`, every subject resolved through `names`.
///
/// `after` names other tasks in the same file, which do not exist until the
/// file has loaded, so it is the one name checked by hand.
fn tasks(text: &str, names: &Names, facts: &Facts) -> Result<Registry<QuestDef>, ContentError> {
    let authored: Registry<QuestRon> = names.load(text)?;
    authored.validate(|q, all| {
        if let Some(a) = q.after.iter().find(|a| all.id(a).is_none()) {
            return Err(format!("after unknown task {a:?}"));
        }
        if q.objectives.is_empty() {
            return Err("no objectives".into());
        }
        Ok(())
    })?;
    let defs = authored
        .iter()
        .map(|(_, q)| QuestDef {
            name: q.name.clone(),
            title: q.title.clone(),
            text: q.text.clone(),
            after: q.after.iter().map(|a| rl_engine::rl_core::Id::from_raw(authored.expect(a).raw())).collect(),
            objectives: q.objectives.iter().map(|o| Objective { text: o.text.clone(), on: o.on.matcher(facts), need: o.need }).collect(),
            victory: q.victory,
        })
        .collect();
    Registry::from_defs(defs)
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

pub fn ledger_keys(keys: ControlInput, binds: Res<Binds>, mut screen: ResMut<LedgerScreen>, mut modals: ResMut<Modals>) {
    let ledger = modal(&modals);
    if keys.just_pressed(binds.ledger) && (modals.is_top(ledger) || !modals.any_open()) {
        modals.toggle(ledger);
        return;
    }
    if !modals.is_top(ledger) {
        return;
    }
    if keys.input().just_pressed(keys.bindings().cursor.close) {
        modals.close_one(ledger);
    }
    if keys.just_pressed(binds.menu_down) {
        screen.menu.move_by(1);
    }
    if keys.just_pressed(binds.menu_up) {
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
        let sea_legs = &quests.defs.get(quests.defs.expect("sea_legs")).objectives[0];
        let beasts = loaded.registries.factions.expect("beasts");
        assert_eq!(sea_legs.on, Matcher::any(facts.killed_faction).about(u64::from(beasts.raw())));
    }

    #[test]
    fn a_task_that_names_what_nobody_registered_is_refused_naming_the_task() {
        let loaded = crate::rules::load(RunSeed(1), Point::ZERO, &crate::rules::effect_kinds());
        let names = loaded.registries.names().with("monster", &loaded.bestiary.defs).with("item", &loaded.armory.defs);
        let bad = r#"[
            (name: "a", title: "A", text: "", objectives: [(text: "", on: Killed("kraken"), need: Total(1))]),
            (name: "b", title: "B", text: "", after: ["c"], objectives: [(text: "", on: Carrying("gold"), need: Latest(1))]),
            (name: "any", title: "Any", text: "", objectives: [(text: "", on: Killed(None), need: Total(1))]),
        ]"#;
        let Err(ContentError::Invalid(errs)) = tasks(bad, &names, &loaded.facts) else { panic!("a file of unknown names loaded") };
        assert!(errs.contains(&"a: unknown monster \"kraken\"".to_string()), "{errs:#?}");
        assert!(errs.contains(&"b: unknown item \"gold\"".to_string()), "{errs:#?}");
        let fine = r#"[(name: "any", title: "Any", text: "", objectives: [(text: "", on: Killed(None), need: Total(1))])]"#;
        let any = tasks(fine, &names, &loaded.facts).expect("a subject left out counts any");
        assert_eq!(any.get(any.expect("any")).objectives[0].on, Matcher::any(loaded.facts.killed));
    }
}
