//! Statuses: the definitions from RON, what a monster's hit inflicts, and
//! the words for it.

use bevy::prelude::*;
use rand::Rng;
use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_rules::{Names, Registry, StatusDef, status};
use rl_engine::rl_ui::{MessageLog, Tones};

use crate::monsters::{Bestiary, MonsterKind};

const STATUSES_RON: &str = include_str!("../assets/statuses.ron");

/// Loads the statuses, their stats and damage kinds named the way the rest
/// of Corsair's content names them; panics listing every problem.
pub fn load(names: &Names) -> Registry<StatusDef> {
    status::load(STATUSES_RON, names).unwrap_or_else(|e| panic!("assets/statuses.ron: {e}"))
}

/// A monster's hit that landed may leave its status behind.
pub fn inflict_on_hit(
    mut dealt: MessageReader<DamageDealt>,
    mut afflict: MessageWriter<Afflict>,
    bestiary: Res<Bestiary>,
    mut rng: ResMut<CombatRng>,
    kinds: Query<&MonsterKind>,
) {
    for d in dealt.read() {
        if d.dealt <= 0 {
            continue;
        }
        let Some(attacker) = d.hit.attacker else { continue };
        let Ok(kind) = kinds.get(attacker) else { continue };
        let Some((status, turns, pct)) = &bestiary.defs.get(kind.0).inflicts else { continue };
        if rng.random_range(0..100) < *pct {
            afflict.write(Afflict { target: d.target, status: status.id(), turns: *turns, by: Some(attacker) });
        }
    }
}

/// Turns status events on the player into log lines.
pub fn narrate_statuses(
    mut events: MessageReader<StatusEvent>,
    registries: Res<Registries>,
    turns: Res<Turns>,
    mut log: ResMut<MessageLog>,
    players: Query<(), With<Player>>,
) {
    let turn = turns.turn_number();
    for ev in events.read() {
        let (target, text, cat) = match *ev {
            StatusEvent::Applied { target, status } => (target, format!("You are {}.", describe(registries.statuses.name(status))), Tones::BAD),
            StatusEvent::Expired { target, status } => (target, format!("You are no longer {}.", describe(registries.statuses.name(status))), Tones::MUTED),
            StatusEvent::Cured { target, status } => (target, format!("The {} passes.", registries.statuses.name(status)), Tones::GOOD),
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
