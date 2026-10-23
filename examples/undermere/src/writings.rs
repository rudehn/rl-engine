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

use atglyph::ecs::PlaceMap;
use atglyph::ecs::places::{PlaceEntered, Spot};
use atglyph::ecs::prefabs::Prefabs;
use atglyph::prelude::*;
use atglyph::rand::Rng;
use atglyph::rules::prefab::Slot;
use bevy::prelude::*;
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
        let mut rng = seed.place(ev.map).rng(crate::seeds::WRITINGS, 0);
        assign(&mut rng, ev.map.0, &spots, (content.last_words.len(), content.inscriptions.len()), &mut progress);
    }
}

/// Gives each of a map's readable `spots` a line, `(position, is last
/// words)` as [`readable_spots`] reports them, from pools of `pools`
/// lines (last words, then inscriptions): skipping a position that has
/// one already, so a rebuilt floor keeps what it had, and drawing the rest
/// in the spots' order so one stream gives the same lines every time.
/// Lifted out of [`assign_writings`] so the rule is tested on made-up
/// spots, with no world to build.
fn assign(rng: &mut impl Rng, map: u32, spots: &[(Point, bool)], pools: (usize, usize), progress: &mut WritingsProgress) {
    for &(at, is_last_words) in spots {
        let key = (map, at.x, at.y);
        if is_last_words {
            if !progress.last_words.contains_key(&key) {
                let line = draw(rng, &mut progress.last_words_used, pools.0);
                progress.last_words.insert(key, line);
            }
        } else if !progress.inscriptions.contains_key(&key) {
            let line = draw(rng, &mut progress.inscriptions_used, pools.1);
            progress.inscriptions.insert(key, line);
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
        if let Some(says) = feeling_of(&place.spots, |spot| piece_prop_at(spot, &prefabs, &registries), &content.feelings) {
            tell.write(Tell::new(says, Tones::NOTICE));
        }
    }
}

/// The name of the prop a piece stands at `spot`: the slot its mark is in
/// the piece or dressing its key names, when that slot is a prop. `None`
/// for a spot with no key, as a trap `traps.rs` laid has none (it is
/// hidden, so a floor never tells of it), whatever its mark would read as
/// in a piece, and for a slot that stands something else. Lifted out of
/// [`tell_floor_feeling`] so what a laid trap reads as is tested on the
/// real pieces.
fn piece_prop_at<'r>(spot: &Spot, prefabs: &Prefabs<MonsterDef>, registries: &'r Registries) -> Option<&'r str> {
    let (key, glyph) = spot.prefab.zip(char::from_u32(spot.tag))?;
    match prefabs.slot(key, glyph)? {
        Slot::Prop(id) => Some(registries.props.name(*id)),
        _ => None,
    }
}

/// What a floor's own feeling says, if a spot of it stands a prop that
/// `feelings` names: the first such spot's, in the spots' order, since a
/// floor is never written to hold two. `prop_name` answers what prop a spot stands, `None` for a spot
/// that stands none, as a trap `traps.rs` laid does not (it is hidden, so
/// a floor never tells of it). Lifted out of [`tell_floor_feeling`] so the
/// rule is tested on made-up spots.
fn feeling_of<'a, 'n>(spots: &[Spot], prop_name: impl Fn(&Spot) -> Option<&'n str>, feelings: &'a [Feeling]) -> Option<&'a str> {
    spots.iter().filter_map(prop_name).find_map(|name| feelings.iter().find(|f| f.prop == name)).map(|f| f.says.as_str())
}

#[cfg(test)]
mod tests {
    use atglyph::primitives::RunSeed;

    use super::*;
    use crate::dungeon::{FLOORS, map_of};
    use crate::testing::{adventurer, arrive, first_floor_holding, first_floor_holding_all, headless, said};

    /// A line is never drawn twice while the pool holds an unused one,
    /// and a pool of one line is read from forever after its one line is
    /// spent, rather than refusing: a run long enough to ask for more
    /// lines than the file holds repeats one instead of failing.
    #[test]
    fn a_line_is_never_drawn_twice_while_the_pool_holds_one_still_unused() {
        let mut rng = RunSeed(0).rng(atglyph::primitives::SeedDomain::new(b"test"), 0);
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

    /// Made-up readable spots: `(position, is last words)` along a row.
    fn row(kinds: &[bool]) -> Vec<(Point, bool)> {
        kinds.iter().enumerate().map(|(i, k)| (Point::new(i as i32, 0), *k)).collect()
    }

    /// No two places are given one line while the pool lasts, across the
    /// floors of a run that share one `progress`, and the two pools are
    /// spent apart: a remains takes nothing from the inscriptions.
    #[test]
    fn no_two_places_in_a_run_are_given_one_line_while_the_pool_lasts() {
        let mut rng = RunSeed(7).rng(atglyph::primitives::SeedDomain::new(b"test"), 0);
        let mut progress = WritingsProgress::default();
        for map in 1..=4 {
            assign(&mut rng, map, &row(&[true, false, true]), (8, 8), &mut progress);
        }
        let last_words: BTreeSet<usize> = progress.last_words.values().copied().collect();
        let inscriptions: BTreeSet<usize> = progress.inscriptions.values().copied().collect();
        assert_eq!((progress.last_words.len(), last_words.len()), (8, 8), "eight remains, eight lines of eight: {:?}", progress.last_words);
        assert_eq!((progress.inscriptions.len(), inscriptions.len()), (4, 4), "four inscriptions, four distinct lines: {:?}", progress.inscriptions);
    }

    /// A place that has a line keeps it when its floor is assigned again,
    /// and takes no further line from the pool, so a floor rebuilt from a
    /// save reads what it read before.
    #[test]
    fn a_place_keeps_its_line_when_its_floor_is_assigned_again() {
        let mut rng = RunSeed(7).rng(atglyph::primitives::SeedDomain::new(b"test"), 0);
        let mut progress = WritingsProgress::default();
        let spots = row(&[true, false]);
        assign(&mut rng, 3, &spots, (5, 5), &mut progress);
        let given = progress.clone();
        assign(&mut rng, 3, &spots, (5, 5), &mut progress);
        assert_eq!(progress, given, "a second assignment of the same floor changed what it was given");
    }

    /// A made-up floor's spots: tag 1 stands a prop named for a feeling,
    /// tag 2 a prop with none, and any other tag nothing.
    fn feeling_spots(tags: &[u32]) -> Vec<Spot> {
        tags.iter().enumerate().map(|(i, tag)| Spot { tag: *tag, at: Point::new(i as i32, 0), prefab: Some(0) }).collect()
    }

    fn name_of_tag(spot: &Spot) -> Option<&'static str> {
        match spot.tag {
            1 => Some("brazier"),
            2 => Some("urn"),
            _ => None,
        }
    }

    fn feelings() -> Vec<Feeling> {
        vec![Feeling { prop: "brazier".into(), says: "smoke".into() }]
    }

    /// A floor with a spot standing a feeling prop says it; one standing
    /// only other props, or nothing, says none.
    #[test]
    fn a_floor_says_its_feeling_only_for_a_spot_that_stands_a_feeling_piece() {
        let feelings = feelings();
        assert_eq!(feeling_of(&feeling_spots(&[2, 1]), name_of_tag, &feelings), Some("smoke"));
        assert_eq!(feeling_of(&feeling_spots(&[2, 4]), name_of_tag, &feelings), None, "a prop the file names no feeling for");
        assert_eq!(feeling_of(&[], name_of_tag, &feelings), None);
    }

    /// A trap `traps.rs` laid stands no piece's prop, so a floor never
    /// tells a feeling for it: its spot carries no key, and reads as
    /// nothing even where its mark is the very glyph a piece stands a
    /// feeling prop at, or the private-use mark the trap laying gives it.
    /// The same glyph keyed to the piece reads as that prop.
    #[test]
    fn a_laid_trap_stands_no_pieces_prop_whatever_its_mark_is() {
        let (prefabs, ..) = crate::prefabs::load(&crate::dungeon::registry(), &crate::dungeon::layout());
        let registries = crate::content::registries();
        let vestibule = prefabs.defs().id("The Paralysis Vestibule").expect("the vestibule is a piece").raw();
        let spot = |tag: u32, prefab: Option<u32>| Spot { tag, at: Point::new(0, 0), prefab };
        assert_eq!(piece_prop_at(&spot('v' as u32, Some(vestibule)), &prefabs, &registries), Some("paralysis vent"), "the piece's own vent");
        assert_eq!(piece_prop_at(&spot('v' as u32, None), &prefabs, &registries), None, "the vent's glyph with no piece");
        assert_eq!(piece_prop_at(&spot(crate::traps::FIRST_TAG, None), &prefabs, &registries), None, "a laid trap's own mark");
        let vent = vec![Feeling { prop: "paralysis vent".into(), says: "still".into() }];
        assert_eq!(feeling_of(&[spot('v' as u32, None)], |s| piece_prop_at(s, &prefabs, &registries), &vent), None, "no feeling for a laid trap");
    }

    /// A floor that stands two feeling props says the one its first spot
    /// stands, and the file's order breaks no tie between spots.
    #[test]
    fn a_floor_with_two_feeling_pieces_says_the_first_spots() {
        let two = vec![Feeling { prop: "urn".into(), says: "dust".into() }, Feeling { prop: "brazier".into(), says: "smoke".into() }];
        assert_eq!(feeling_of(&feeling_spots(&[1, 2]), name_of_tag, &two), Some("smoke"));
        assert_eq!(feeling_of(&feeling_spots(&[2, 1]), name_of_tag, &two), Some("dust"));
    }

    /// Every "scratched words" prop one floor turns up, read once each: the
    /// same line read twice running, and no two of them the same line, on
    /// one floor that holds both pieces that write, so at least two remains
    /// are asked to differ. One floor is enough here: the reading is the
    /// test, and that no two places across a run share a line is the rule
    /// `assign` is tested for above.
    #[test]
    fn a_remains_reads_the_same_line_twice_and_no_two_remains_on_a_floor_share_one() {
        let pieces = ["A Fallen Adventurer", "Last Words"];
        let (seed, f) = first_floor_holding_all(&pieces);
        let at = format!("{pieces:?} on seed {seed:?}, floor {f}");
        let mut app = headless(seed);
        let me = adventurer(&mut app);
        arrive(&mut app, map_of(f));
        let scratched = app.world().resource::<Registries>().props.expect("scratched words");
        let read = app.world().resource::<Verbs>().get(READ_VERB).expect("declared");
        let found: Vec<Entity> = {
            let world = app.world_mut();
            let here = world.resource::<WorldMap>().current();
            world.query::<(Entity, &PropKind, &OnMap)>().iter(world).filter(|(_, k, on)| k.0 == scratched && on.0 == here).map(|(e, ..)| e).collect()
        };
        assert!(found.len() >= 2, "{at}: a remains for each piece, {} found", found.len());
        let mut lines_this_floor = Vec::new();
        for target in found {
            let before_first = said(&app).len();
            app.world_mut().write_message(Interacted { actor: me, target, verb: read, thing: None });
            app.update();
            let first_read = said(&app)[before_first..].to_vec();
            assert_eq!(first_read.len(), 1, "{at}: reading told something other than one line: {first_read:?}");
            let before_second = said(&app).len();
            app.world_mut().write_message(Interacted { actor: me, target, verb: read, thing: None });
            app.update();
            let second_read = said(&app)[before_second..].to_vec();
            // The log folds a line said twice running into the one,
            // so a second read adds a new line or none, never another.
            assert!(second_read.is_empty() || first_read == second_read, "{at}: the same remains read twice");
            lines_this_floor.push(first_read.into_iter().next().expect("checked above"));
        }
        let distinct: BTreeSet<&String> = lines_this_floor.iter().collect();
        assert_eq!(distinct.len(), lines_this_floor.len(), "{at}: a line given to two remains on one floor: {lines_this_floor:?}");
    }

    /// Whether the floor the adventurer stands on says a feeling on
    /// arrival, and whether a feeling piece (not a laid trap) stands on it.
    fn feeling_said_and_held(app: &mut App, f: u32) -> (bool, bool) {
        let before = said(app).len();
        arrive(app, map_of(f));
        let new_lines = said(app)[before..].to_vec();
        let feeling_lines: Vec<String> = app.world().resource::<Writings>().feelings.iter().map(|f| f.says.clone()).collect();
        let said_a_feeling = new_lines.iter().any(|line| feeling_lines.iter().any(|f| f == line));
        let registries = app.world().resource::<Registries>().clone();
        let feeling_ids: Vec<PropId> = app.world().resource::<Writings>().feelings.iter().filter_map(|f| registries.props.id(&f.prop)).collect();
        let world = app.world_mut();
        let here = world.resource::<WorldMap>().current();
        // A trap `traps.rs` lays is a prop of the same kind but no piece:
        // it is hidden, so a floor never tells of it.
        let laid: Vec<Point> =
            world.resource::<WorldMap>().place(here).expect("built").spots.iter().filter(|s| crate::traps::is_hidden_tag(s.tag)).map(|s| s.at).collect();
        let holds = world
            .query::<(&PropKind, &Position, &OnMap)>()
            .iter(world)
            .filter(|(_, at, on)| on.0 == here && !laid.contains(&at.0))
            .any(|(k, ..)| feeling_ids.contains(&k.0));
        (said_a_feeling, holds)
    }

    /// A floor holding a piece `Writings::feelings` names says it on
    /// arrival; one holding none of them says none of those lines. One
    /// run is enough: the first floor drawing "The Tinder Room" is staged,
    /// then the same run's other floors are searched for the first without
    /// a feeling piece, and the search fails loudly when there is none.
    #[test]
    fn a_floor_with_a_feeling_piece_says_it_on_arrival_and_one_without_says_none() {
        let (seed, f) = first_floor_holding("The Tinder Room");
        let mut app = headless(seed);
        adventurer(&mut app);
        let (said_a_feeling, holds) = feeling_said_and_held(&mut app, f);
        assert!(holds, "seed {seed:?}, floor {f}: The Tinder Room was drawn but no feeling piece stands here");
        assert!(said_a_feeling, "seed {seed:?}, floor {f}: a feeling piece stands here, unannounced");
        let bare = (1..FLOORS).filter(|g| *g != f).find(|g| {
            let (said_a_feeling, holds) = feeling_said_and_held(&mut app, *g);
            assert_eq!(said_a_feeling, holds, "seed {seed:?}, floor {g}: a feeling said with no feeling piece here, or none said with one");
            !holds
        });
        assert!(bare.is_some(), "seed {seed:?}: every floor of the run held a feeling piece, so none was left to say none");
    }
}
