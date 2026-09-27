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

/// The stream the "does the bite poison you" roll comes from: the game's
/// own, from `Seed::stream(b"corsair.inflicts", 0)`, seeded once when the
/// run starts.
///
/// Never the engine's own `CombatRng`, which this drew from until
/// 2026-09-22: a roll a game adds must not shift the dice of the blows
/// the engine has yet to throw.
#[derive(Resource)]
pub struct Inflicts(pub rand::rngs::StdRng);

/// A monster's hit that landed may leave its status behind.
pub fn inflict_on_hit(
    mut dealt: MessageReader<DamageDealt>,
    mut afflict: MessageWriter<Afflict>,
    bestiary: Res<Bestiary>,
    mut rng: ResMut<Inflicts>,
    kinds: Query<&MonsterKind>,
) {
    for d in dealt.read() {
        if d.dealt <= 0 {
            continue;
        }
        let Some(attacker) = d.hit.attacker else { continue };
        let Ok(kind) = kinds.get(attacker) else { continue };
        let Some((status, turns, pct)) = &bestiary.defs.get(kind.0).inflicts else { continue };
        if rng.0.random_range(0..100) < *pct {
            afflict.write(Afflict { target: d.target, status: status.id(), turns: *turns, by: Some(attacker), held_by: None });
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
            StatusEvent::Applied { target, status } => {
                let tone = if registries.statuses.get(status).boon { Tones::GOOD } else { Tones::BAD };
                (target, format!("You are {}.", describe(registries.statuses.name(status))), tone)
            }
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

#[cfg(test)]
mod tests {
    use rl_engine::rl_core::RunSeed;

    use super::*;
    use crate::items::{Armory, ItemKind};

    /// Drinking the bottle puts heart in the captain, and the log says so
    /// once, in the good tone: Corsair's own line, with every engine
    /// phrase for a status on the player silenced, a boon's included.
    #[test]
    fn drinking_the_bottle_says_you_are_hearty_once_as_good_news() {
        let dir = std::env::temp_dir().join(format!("corsair-hearty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = crate::testing::headless(RunSeed(7), false, &dir);
        app.add_plugins(crate::narrator()).add_systems(Update, narrate_statuses.in_set(PresentSet::Narrate));
        app.update();
        app.update();
        let me = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
        let rum_kind = app.world().resource::<Armory>().defs.expect("bottle of rum");
        let rum = {
            let w = app.world();
            w.get::<Inventory>(me)
                .expect("a bag")
                .items
                .iter()
                .copied()
                .find(|i| w.get::<ItemKind>(*i).is_some_and(|k| k.0 == rum_kind))
                .expect("a bottle of rum")
        };
        app.world_mut().write_message(Intent::new(me, UseItem(rum)));
        app.update();
        app.update();

        let hearty: Vec<_> = app.world().resource::<MessageLog>().iter().filter(|e| e.text == "You are hearty.").map(|e| (e.count, e.tone)).collect();
        assert_eq!(hearty, vec![(1, Tones::GOOD)], "one line, said once, as good news");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
