//! Statuses: the definitions from RON, what a monster's hit inflicts, and
//! the words for it.

use bevy::prelude::*;
use rand::Rng;
use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_rules::{Named, Registry};
use rl_engine::rl_rules::{Op, Stacking, StatusDef};
use rl_engine::rl_ui::{LogCategory, MessageLog};
use serde::Deserialize;

use crate::items::Armory;
use crate::monsters::{Bestiary, MonsterKind};

const STATUSES_RON: &str = include_str!("../assets/statuses.ron");

/// A status as authored.
#[derive(Debug, Clone, Deserialize)]
struct StatusRon {
    name: String,
    #[serde(default = "refresh")]
    stacking: Stacking,
    #[serde(default)]
    modifies: Vec<(String, Op)>,
    #[serde(default)]
    ticks: Option<(String, i32)>,
    #[serde(default)]
    badge: Option<char>,
}

fn refresh() -> Stacking {
    Stacking::Refresh
}

impl Named for StatusRon {
    fn name(&self) -> &str {
        &self.name
    }
}

/// Loads the statuses with stat and damage kind names resolved; panics
/// listing every problem.
pub fn load(armory: &Armory, bestiary: &Bestiary) -> StatusRules {
    let authored: Registry<StatusRon> = Registry::from_ron_str(STATUSES_RON).unwrap_or_else(|e| panic!("assets/statuses.ron: {e}"));
    authored
        .validate(|s, _| {
            for (stat, _) in &s.modifies {
                if armory.stats.id(stat).is_none() {
                    return Err(format!("{}: unknown stat {stat:?}", s.name));
                }
            }
            if let Some((kind, _)) = &s.ticks
                && bestiary.kinds.id(kind).is_none()
            {
                return Err(format!("{}: unknown damage kind {kind:?}", s.name));
            }
            Ok(())
        })
        .unwrap_or_else(|e| panic!("assets/statuses.ron: {e}"));
    let defs = authored
        .iter()
        .map(|(_, s)| StatusDef {
            name: s.name.clone(),
            stacking: s.stacking,
            modifiers: s
                .modifies
                .iter()
                .map(|(stat, op)| rl_engine::rl_rules::status::StatusModifier { stat: armory.stats.expect(stat).raw(), op: *op })
                .collect(),
            tick_damage: s.ticks.as_ref().map(|(kind, n)| (bestiary.kinds.expect(kind).raw(), *n)),
            badge: s.badge,
        })
        .collect();
    StatusRules { defs: Registry::from_defs(defs).unwrap() }
}

/// A monster's hit that landed may leave its status behind.
pub fn inflict_on_hit(
    mut dealt: MessageReader<DamageDealt>,
    mut afflict: MessageWriter<Afflict>,
    bestiary: Res<Bestiary>,
    rules: Res<StatusRules>,
    mut rng: ResMut<CombatRng>,
    kinds: Query<&MonsterKind>,
) {
    for d in dealt.read() {
        if d.dealt <= 0 {
            continue;
        }
        let Some(attacker) = d.hit.attacker else { continue };
        let Ok(kind) = kinds.get(attacker) else { continue };
        let Some((name, turns, pct)) = &bestiary.defs.get(kind.0).inflicts else { continue };
        if rng.random_range(0..100) < *pct {
            afflict.write(Afflict { target: d.target, status: rules.defs.expect(name), turns: *turns, by: Some(attacker) });
        }
    }
}

/// Turns status events on the player into log lines.
pub fn narrate_statuses(
    mut events: MessageReader<StatusEvent>,
    rules: Res<StatusRules>,
    turns: Res<Turns>,
    mut log: ResMut<MessageLog>,
    players: Query<(), With<Player>>,
) {
    let turn = turns.turn_number();
    for ev in events.read() {
        let (target, text, cat) = match *ev {
            StatusEvent::Applied { target, status } => (target, format!("You are {}.", describe(rules.defs.name(status))), LogCategory::Bad),
            StatusEvent::Expired { target, status } => (target, format!("You are no longer {}.", describe(rules.defs.name(status))), LogCategory::Muted),
            StatusEvent::Cured { target, status } => (target, format!("The {} passes.", rules.defs.name(status)), LogCategory::Good),
        };
        if players.get(target).is_ok() {
            log.push(text, cat, turn);
        }
    }
}

fn describe(name: &str) -> &str {
    match name {
        "venom" => "poisoned",
        _ => name,
    }
}

/// The badges of the player's statuses, for the status line.
pub fn badges(statuses: &Afflicted, rules: &StatusRules) -> String {
    statuses.iter().filter_map(|s| rules.defs.get(s.id).badge.map(|b| format!("{b}{}", s.turns))).collect::<Vec<_>>().join(" ")
}
