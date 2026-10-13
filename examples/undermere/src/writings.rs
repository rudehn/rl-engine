//! What the dead left behind, and what the drowned kingdom's history
//! reads like carved into a wall: `assets/writings.ron`, the lines
//! [`assign_writings`] hands out, and the verb [`answer_read`] answers.
//!
//! A "scratched words" prop beside a body is a dead adventurer's last
//! line; a "chronicle" or a "wall inscription" is the kingdom's own
//! history. [`assign_writings`] runs on the arrival that first finds a
//! place, after the engine's own `PrefabSet::Fill`, and gives every such
//! prop on it a line of [`Writings`]'s own, drawn without replacement
//! from [`WritingsProgress`]'s own running account of what this run has
//! already given out, so a line is never read twice in one run. The
//! line is kept by the prop's own position rather than by a component
//! on it, so it survives a save exactly as the prop's kind and whether
//! it is spotted already do.
//!
//! [`tell_floor_feeling`] says a floor's own feeling once, the same
//! arrival: a floor that holds a piece [`Writings::feelings`] names is
//! said to smell of smoke or sound of water before anyone has found
//! what is making it, the rare floor research 8.2 calls anticipated
//! rather than merely found.

use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;
use rand::Rng;
use rl_engine::prelude::*;
use rl_engine::rl_bevy::PlaceMap;
use rl_engine::rl_bevy::places::PlaceEntered;
use rl_engine::rl_bevy::prefabs::Prefabs;
use rl_engine::rl_rules::prefab::Slot;
use serde::{Deserialize, Serialize};

use crate::bestiary::MonsterDef;

/// What a reward piece, a flavor piece or `decor::WallInscriptions`
/// offers to be read, declared by [`crate::plugin::UndermerePlugin`].
pub const READ_VERB: &str = "read";

/// The writings file, compiled in so the binary runs from anywhere.
const WRITINGS_RON: &str = include_str!("../assets/writings.ron");

/// One row of `writings.ron`'s own `feelings`: a prop whose presence on
/// a floor is worth saying before it is found.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Feeling {
    /// The prop's name, from `props.ron`.
    pub prop: String,
    /// What the floor's arrival says.
    pub says: String,
}

/// `assets/writings.ron`, read once at startup: every line a remains or
/// an inscription may be given, and every floor's own feeling.
#[derive(Debug, Clone, Deserialize, Resource)]
#[serde(deny_unknown_fields)]
pub struct Writings {
    /// Lines a dead adventurer scratched beside where they fell.
    pub last_words: Vec<String>,
    /// Lines of the drowned kingdom's history.
    pub inscriptions: Vec<String>,
    /// A prop whose presence on a floor says something on arrival.
    pub feelings: Vec<Feeling>,
}

/// Reads `assets/writings.ron`. Panics, by name, on a file that does not
/// parse: a run never starts on writings it cannot read.
pub fn load() -> Writings {
    ron::de::from_str(WRITINGS_RON).unwrap_or_else(|e| panic!("assets/writings.ron: {e}"))
}

/// Which line a remains or an inscription at a position was given, and
/// which lines this run has given out already, so neither repeats. Kept
/// by the prop's own map and position, as the engine already keeps
/// whether a prop is spotted, rather than by a component on it, so a
/// save round-trips it without this module asking the save for anything
/// of its own.
#[derive(Resource, Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WritingsProgress {
    /// A "scratched words" prop's line, by its map and position.
    last_words: BTreeMap<(u32, i32, i32), usize>,
    /// Every `last_words` line index given out this run.
    last_words_used: BTreeSet<usize>,
    /// A "chronicle" or "wall inscription" prop's line, by its map and
    /// position.
    inscriptions: BTreeMap<(u32, i32, i32), usize>,
    /// Every `inscriptions` line index given out this run.
    inscriptions_used: BTreeSet<usize>,
}

impl SaveableState for WritingsProgress {
    type Saved = WritingsProgress;

    fn capture(&self) -> WritingsProgress {
        self.clone()
    }

    fn restore(&mut self, saved: WritingsProgress) {
        *self = saved;
    }
}

/// A line of `pool`'s own, drawn without replacing one already in
/// `used` while any are left; a run that asks for more lines than the
/// file holds repeats rather than failing, since nothing stops a very
/// long one asking.
fn draw(rng: &mut impl Rng, used: &mut BTreeSet<usize>, pool_len: usize) -> usize {
    if pool_len == 0 {
        return 0;
    }
    let left: Vec<usize> = (0..pool_len).filter(|i| !used.contains(i)).collect();
    let picked = if left.is_empty() { rng.random_range(0..pool_len) } else { left[rng.random_range(0..left.len())] };
    used.insert(picked);
    picked
}

/// Every "scratched words", "chronicle" and "wall inscription" prop the
/// place's own spots name, and which pool each reads from.
fn readable_spots(place: &PlaceMap, registries: &Registries, prefabs: &Prefabs<MonsterDef>) -> Vec<(Point, bool)> {
    let scratched = registries.props.expect("scratched words");
    let chronicle = registries.props.expect("chronicle");
    let inscription = registries.props.expect("wall inscription");
    let mut spots = Vec::new();
    for spot in &place.spots {
        let Some((key, glyph)) = spot.prefab.zip(char::from_u32(spot.tag)) else { continue };
        let Some(Slot::Prop(id)) = prefabs.slot(key, glyph) else { continue };
        if *id == scratched {
            spots.push((spot.at, true));
        } else if *id == chronicle || *id == inscription {
            spots.push((spot.at, false));
        }
    }
    spots.sort_by_key(|(p, _)| (p.x, p.y));
    spots
}

/// Gives every readable prop a place's first arrival finds a line of
/// `Writings`'s own, from one stream a floor, so two props on the same
/// floor never draw the same way twice. Never one already given to this
/// exact position, so a floor rebuilt from a save keeps what it had.
///
/// In `TurnSet::React`, after `PrefabSet::Fill`.
pub fn assign_writings(
    mut entered: MessageReader<PlaceEntered>,
    map: Res<WorldMap>,
    registries: Res<Registries>,
    prefabs: Res<Prefabs<MonsterDef>>,
    seed: Res<Seed>,
    content: Res<Writings>,
    mut progress: ResMut<WritingsProgress>,
) {
    for ev in entered.read().filter(|ev| ev.first) {
        let Some(place) = map.place(ev.map) else { continue };
        let spots = readable_spots(place, &registries, &prefabs);
        if spots.is_empty() {
            continue;
        }
        let mut rng = seed.stream(b"undermere.writings", u64::from(ev.map.0));
        for (at, is_last_words) in spots {
            let key = (ev.map.0, at.x, at.y);
            if is_last_words {
                if !progress.last_words.contains_key(&key) {
                    let line = draw(&mut rng, &mut progress.last_words_used, content.last_words.len());
                    progress.last_words.insert(key, line);
                }
            } else if !progress.inscriptions.contains_key(&key) {
                let line = draw(&mut rng, &mut progress.inscriptions_used, content.inscriptions.len());
                progress.inscriptions.insert(key, line);
            }
        }
    }
}

/// What answering a read looks at: the verb, the prop kinds, where the
/// adventurer is and the lines given out.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Readings<'w> {
    verbs: Res<'w, Verbs>,
    registries: Res<'w, Registries>,
    map: Res<'w, WorldMap>,
    content: Res<'w, Writings>,
    progress: Res<'w, WritingsProgress>,
}

/// Reading a chronicle, a wall inscription or a dead adventurer's last
/// words says the line [`assign_writings`] gave it, the same one every
/// time it is read again.
///
/// In `TurnSet::React`, where every answer to an `Interacted` is.
pub fn answer_read(mut done: MessageReader<Interacted>, reading: Readings, props: Query<(&PropKind, &Position)>, mut tell: MessageWriter<Tell>) {
    let Some(read) = reading.verbs.get(READ_VERB) else { return };
    let scratched = reading.registries.props.expect("scratched words");
    let chronicle = reading.registries.props.expect("chronicle");
    let inscription = reading.registries.props.expect("wall inscription");
    for ev in done.read().filter(|ev| ev.verb == read) {
        let Ok((kind, at)) = props.get(ev.target) else { continue };
        let key = (reading.map.current().0, at.0.x, at.0.y);
        let line = if kind.0 == scratched {
            reading.progress.last_words.get(&key).and_then(|i| reading.content.last_words.get(*i))
        } else if kind.0 == chronicle || kind.0 == inscription {
            reading.progress.inscriptions.get(&key).and_then(|i| reading.content.inscriptions.get(*i))
        } else {
            None
        };
        if let Some(line) = line {
            tell.write(Tell::new(line, Tones::NOTICE));
        }
    }
}

/// Says a floor's own feeling once, on the arrival that first finds it,
/// when one of its pieces carries one: the first of `Writings::feelings`
/// whose prop stands on the floor, by the order the file lists them in,
/// since a floor is never written to hold two.
///
/// In `TurnSet::React`, after `PrefabSet::Fill`.
pub fn tell_floor_feeling(
    mut entered: MessageReader<PlaceEntered>,
    map: Res<WorldMap>,
    registries: Res<Registries>,
    prefabs: Res<Prefabs<MonsterDef>>,
    content: Res<Writings>,
    mut tell: MessageWriter<Tell>,
) {
    for ev in entered.read().filter(|ev| ev.first) {
        let Some(place) = map.place(ev.map) else { continue };
        for spot in &place.spots {
            let Some((key, glyph)) = spot.prefab.zip(char::from_u32(spot.tag)) else { continue };
            let Some(Slot::Prop(id)) = prefabs.slot(key, glyph) else { continue };
            let name = registries.props.name(*id);
            if let Some(feeling) = content.feelings.iter().find(|f| f.prop == name) {
                tell.write(Tell::new(feeling.says.as_str(), Tones::NOTICE));
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use rl_engine::rl_core::RunSeed;

    use super::*;
    use crate::dungeon::{FLOORS, map_of};
    use crate::testing::{adventurer, arrive, headless, par_seeds, said};
    use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

    /// A line is never drawn twice while the pool holds an unused one,
    /// and a pool of one line is read from forever after its one line is
    /// spent, rather than refusing: a run long enough to ask for more
    /// lines than the file holds repeats one instead of failing.
    #[test]
    fn a_line_is_never_drawn_twice_while_the_pool_holds_one_still_unused() {
        let mut rng = RunSeed(0).rng(rl_engine::rl_core::SeedDomain::new(b"test"), 0);
        let mut used = BTreeSet::new();
        let mut drawn = Vec::new();
        for _ in 0..5 {
            drawn.push(draw(&mut rng, &mut used, 5));
        }
        let distinct: BTreeSet<usize> = drawn.iter().copied().collect();
        assert_eq!(distinct.len(), 5, "five draws against a pool of five, none repeated: {drawn:?}");
        assert!(drawn.iter().all(|i| *i < 5), "every draw is a real index: {drawn:?}");
        // The pool of five is spent; a sixth draw repeats one instead of
        // panicking or indexing out of bounds.
        let sixth = draw(&mut rng, &mut used, 5);
        assert!(sixth < 5, "a spent pool still returns a real index: {sixth}");
    }

    /// Every "scratched words" prop a run's floors turn up, read once
    /// each: the same line read twice running, and no two of them the
    /// same line, across as many floors as the seed range gives one.
    #[test]
    fn a_remains_reads_the_same_line_twice_and_no_two_remains_in_a_run_share_one() {
        par_seeds(0..24, |s| {
            let mut app = headless(RunSeed(s));
            let me = adventurer(&mut app);
            let mut lines_this_run = Vec::new();
            for f in 1..FLOORS {
                arrive(&mut app, map_of(f));
                let scratched = app.world().resource::<Registries>().props.expect("scratched words");
                let read = app.world().resource::<Verbs>().get(READ_VERB).expect("declared");
                let found: Vec<Entity> = {
                    let world = app.world_mut();
                    let here = world.resource::<WorldMap>().current();
                    world.query::<(Entity, &PropKind, &OnMap)>().iter(world).filter(|(_, k, on)| k.0 == scratched && on.0 == here).map(|(e, ..)| e).collect()
                };
                for target in found {
                    let before_first = said(&app).len();
                    app.world_mut().write_message(Interacted { actor: me, target, verb: read, thing: None });
                    app.update();
                    let first_read = said(&app)[before_first..].to_vec();
                    assert_eq!(first_read.len(), 1, "seed {s}, floor {f}: reading told something other than one line: {first_read:?}");
                    let before_second = said(&app).len();
                    app.world_mut().write_message(Interacted { actor: me, target, verb: read, thing: None });
                    app.update();
                    let second_read = said(&app)[before_second..].to_vec();
                    // The log folds a line said twice running into the one,
                    // so a second read adds a new line or none, never another.
                    assert!(second_read.is_empty() || first_read == second_read, "seed {s}, floor {f}: the same remains read twice");
                    lines_this_run.push(first_read.into_iter().next().expect("checked above"));
                }
            }
            let distinct: BTreeSet<&String> = lines_this_run.iter().collect();
            assert_eq!(distinct.len(), lines_this_run.len(), "seed {s}: a line given to two remains in one run: {lines_this_run:?}");
        });
    }

    /// A floor holding a piece `Writings::feelings` names says it on
    /// arrival; one holding none of them says none of those lines. A hidden
    /// trap of the same kind is no piece, and a floor does not give it away.
    #[test]
    fn a_floor_with_a_feeling_piece_says_it_on_arrival_and_one_without_says_none() {
        let (told_one, told_none) = (AtomicBool::new(false), AtomicBool::new(false));
        par_seeds(0..40, |s| {
            let mut app = headless(RunSeed(s));
            adventurer(&mut app);
            for f in 1..FLOORS {
                let before = said(&app).len();
                arrive(&mut app, map_of(f));
                let after = said(&app);
                let new_lines = &after[before..];
                let feeling_lines: Vec<String> = app.world().resource::<Writings>().feelings.iter().map(|f| f.says.clone()).collect();
                let said_a_feeling = new_lines.iter().any(|line| feeling_lines.iter().any(|f| f == line));
                let registries = app.world().resource::<Registries>().clone();
                let feeling_ids: Vec<PropId> = app.world().resource::<Writings>().feelings.iter().filter_map(|f| registries.props.id(&f.prop)).collect();
                let world = app.world_mut();
                let here = world.resource::<WorldMap>().current();
                // A trap `traps.rs` lays is a prop of the same kind but no
                // piece: it is hidden, so a floor never tells of it.
                let laid: Vec<Point> = world
                    .resource::<WorldMap>()
                    .place(here)
                    .expect("built")
                    .spots
                    .iter()
                    .filter(|s| crate::traps::is_hidden_tag(s.tag))
                    .map(|s| s.at)
                    .collect();
                let holds_a_feeling_prop = world
                    .query::<(&PropKind, &Position, &OnMap)>()
                    .iter(world)
                    .filter(|(_, at, on)| on.0 == here && !laid.contains(&at.0))
                    .any(|(k, ..)| feeling_ids.contains(&k.0));
                if holds_a_feeling_prop {
                    assert!(said_a_feeling, "seed {s}, floor {f}: a feeling piece stands here, unannounced");
                    told_one.store(true, Relaxed);
                } else {
                    assert!(!said_a_feeling, "seed {s}, floor {f}: a feeling said with no feeling piece here");
                    told_none.store(true, Relaxed);
                }
            }
        });
        assert!(told_one.into_inner(), "no seed in the range held a feeling piece at all");
        assert!(told_none.into_inner(), "no seed in the range held a floor with none");
    }
}
