//! Whether a game's systems that touch the same data are ordered.
//!
//! Bevy orders systems only by the constraints declared on them. Two that
//! touch the same data, one of them writing it, with no order between
//! them, run in whichever order the scheduler picks, and adding any
//! unrelated system or set anywhere can change the pick: the game then
//! plays differently with nothing in it changed. A missing chain still
//! passes every behaviour test while the scheduler happens to pick the
//! order the systems were declared in; this is what notices.
//!
//! A game's test builds its app twice, with its own plugin and without
//! it, and [`added`] takes the game's systems to be whatever the plugin
//! added, so a system added later is covered without being listed
//! anywhere. [`assert_ordered`] then fails on every conflicting pair with
//! one of them in it that is neither ordered nor [`Allowed`] with a
//! written reason, and on every allowance no pair needs any more, so the
//! list does not outlive what it explains.
//!
//! Systems are recognised by the type of their function, from
//! [`system_type`], or by the sets they sit in, never by name: a system's
//! name is its path only in a build with Bevy's `debug` feature, which the
//! workspace builds with but a game need not. A failure reads by the names
//! a test gives the systems and the data it meets, and by Bevy's own names
//! for the rest.
//!
//! A game names only its own systems in an [`Allowed`], beside the data
//! ([`Allowed::with_any`]) or the engine's sets ([`Allowed::among`]): an
//! engine system function is the engine's to rename, and its pairs with
//! another engine system are the engine's own test's to answer for.

use std::any::TypeId;

use bevy::ecs::component::ComponentId;
use bevy::ecs::schedule::{InternedScheduleLabel, InternedSystemSet, Schedules};
use bevy::prelude::*;

/// A system as the check knows it: the schedule it runs in and the type
/// of its function.
pub type SystemKey = (InternedScheduleLabel, TypeId);

/// Every system in every schedule of `app`, each schedule built first so
/// its conflicts are known.
pub fn systems(app: &mut App) -> Vec<SystemKey> {
    let world = app.world_mut();
    let labels: Vec<_> = world.resource::<Schedules>().iter().map(|(_, schedule)| schedule.label()).collect();
    let mut all = Vec::new();
    for label in labels {
        world.schedule_scope(label, |world, schedule| {
            schedule.initialize(world).expect("every schedule builds");
            all.extend(schedule.systems().expect("just initialized").map(|(_, system)| (label, system.system_type())));
        });
    }
    all
}

/// The systems `with` has beyond those `without` has, each counted, so a
/// system the game registers in a schedule beside the engine's own copy
/// of it is still the game's once. Builds every schedule of both.
pub fn added(with: &mut App, without: &mut App) -> Vec<SystemKey> {
    let mut theirs = systems(without);
    let mut ours = Vec::new();
    for entry in systems(with) {
        match theirs.iter().position(|e| *e == entry) {
            Some(i) => {
                theirs.swap_remove(i);
            }
            None => ours.push(entry),
        }
    }
    ours
}

/// The type a function registers as, to recognise it among a schedule's
/// systems.
pub fn system_type<M>(system: impl IntoSystem<(), (), M>) -> TypeId {
    IntoSystem::into_system(system).system_type()
}

/// The component or resource ids of the data a test names, by name, for
/// an [`Allowed`] to say what it allows and a failure to read by.
///
/// # Panics
/// Panics on one that is not registered, which a schedule that touches it
/// would have done once built.
pub fn data(found: impl IntoIterator<Item = (&'static str, Option<ComponentId>)>) -> Vec<(&'static str, ComponentId)> {
    found.into_iter().map(|(name, id)| (name, id.unwrap_or_else(|| panic!("{name} is registered once every schedule is built")))).collect()
}

/// The ids in `data` named `names`, in that order.
///
/// # Panics
/// Panics on a name `data` does not hold.
pub fn on(data: &[(&'static str, ComponentId)], names: &[&str]) -> Vec<ComponentId> {
    names.iter().map(|n| data.iter().find(|(name, _)| name == n).unwrap_or_else(|| panic!("{n} is named in the data")).1).collect()
}

/// Two systems that may conflict on `on` without being ordered, and why
/// that is harmless. `None` stands for any system.
///
/// A conflict is allowed only when every piece of data it is over is in
/// `on`, so an allowance written for a pair's shared message does not
/// quietly cover the component they come to share later.
pub struct Allowed {
    /// One of the two.
    pub a: Option<TypeId>,
    /// The other.
    pub b: Option<TypeId>,
    /// The sets both must sit in, each in one of them, directly or through
    /// a set inside it; empty for anywhere.
    pub among: Vec<InternedSystemSet>,
    /// What they may conflict over.
    pub on: Vec<ComponentId>,
    /// Why their order cannot matter.
    pub why: &'static str,
}

impl Allowed {
    /// `a` and `b`, over `on`.
    pub fn pair(a: TypeId, b: TypeId, on: Vec<ComponentId>, why: &'static str) -> Self {
        Self { a: Some(a), b: Some(b), among: Vec::new(), on, why }
    }

    /// `a` and any other system, over `on`.
    pub fn with_any(a: TypeId, on: Vec<ComponentId>, why: &'static str) -> Self {
        Self { a: Some(a), b: None, among: Vec::new(), on, why }
    }

    /// Any two systems that both sit in `sets`, over `on`: what a set
    /// promises of every system in it, such as one actor's one action a
    /// pass among the resolvers, admitted without naming a system, and
    /// nothing outside the sets admitted by it.
    pub fn among(sets: Vec<InternedSystemSet>, on: Vec<ComponentId>, why: &'static str) -> Self {
        Self { a: None, b: None, among: sets, on, why }
    }

    /// Any two systems, over `on`.
    pub fn any(on: Vec<ComponentId>, why: &'static str) -> Self {
        Self { a: None, b: None, among: Vec::new(), on, why }
    }
}

/// Fails unless every pair of systems in `world`'s schedules that
/// conflict with no order between them, at least one of them in `ours`,
/// is [`Allowed`], and unless every allowance is still needed by a pair.
///
/// `game` names whose systems these are in the message; `names` and
/// `data` name the systems and the data a failure reads by. Anything they
/// do not name is printed as Bevy names it, which is its full path in a
/// build with Bevy's `debug` feature and a placeholder beside its id
/// without, so a pair a test has never met can be read by building once
/// with `--features bevy/debug`. A conflict over the whole world, an
/// exclusive system's, comes with no data, and no allowance covers it.
///
/// # Panics
/// Panics with every unexplained pair, or with every allowance no pair
/// needs, so one run lists everything to fix.
pub fn assert_ordered(world: &World, game: &str, ours: &[SystemKey], allowed: &[Allowed], names: &[(&str, TypeId)], data: &[(&str, ComponentId)]) {
    let mut bevy_names: Vec<(TypeId, String)> = Vec::new();
    for (_, schedule) in world.resource::<Schedules>().iter() {
        bevy_names.extend(schedule.systems().expect("initialized").map(|(_, system)| (system.system_type(), system.name().to_string())));
    }
    let system = |t: TypeId| {
        names.iter().find(|(_, n)| *n == t).map(|(n, _)| n.to_string()).unwrap_or_else(|| {
            let bevy = bevy_names.iter().find(|(n, _)| *n == t).map_or("", |(_, name)| name.as_str());
            format!("{bevy} {t:?}")
        })
    };
    let datum = |c: &ComponentId| {
        data.iter().find(|(_, id)| id == c).map(|(n, _)| n.to_string()).unwrap_or_else(|| {
            let bevy = world.components().get_info(*c).map(|info| info.name().to_string()).unwrap_or_default();
            format!("{bevy} {c:?}")
        })
    };
    let fits = |rule: Option<TypeId>, t: TypeId| rule.is_none_or(|r| r == t);
    let mut used = vec![false; allowed.len()];
    let mut unexplained = Vec::new();
    for (_, schedule) in world.resource::<Schedules>().iter() {
        let label = schedule.label();
        let types: Vec<_> = schedule.systems().expect("initialized").map(|(key, system)| (key, system.system_type())).collect();
        let type_of = |k| types.iter().find(|(key, _)| *key == k).map(|(_, t)| *t).expect("a system of this schedule");
        let graph = schedule.graph();
        // A set this schedule has no system in holds nothing here.
        let inside = |rule: &Allowed, k| rule.among.is_empty() || rule.among.iter().any(|set| graph.systems_in_set(*set).is_ok_and(|keys| keys.contains(&k)));
        for (ka, kb, on) in graph.conflicting_systems().iter() {
            let (a, b) = (type_of(*ka), type_of(*kb));
            if !ours.contains(&(label, a)) && !ours.contains(&(label, b)) {
                continue;
            }
            let rule = allowed.iter().position(|r| {
                ((fits(r.a, a) && fits(r.b, b)) || (fits(r.a, b) && fits(r.b, a)))
                    && inside(r, *ka)
                    && inside(r, *kb)
                    && !on.is_empty()
                    && on.iter().all(|c| r.on.contains(c))
            });
            match rule {
                Some(i) => used[i] = true,
                None => {
                    let what: Vec<String> = on.iter().map(datum).collect();
                    unexplained.push(format!("{label:?}: {} and {} on {what:?}", system(a), system(b)));
                }
            }
        }
    }
    assert!(
        unexplained.is_empty(),
        "{} unordered conflicting pairs of {game}'s; order them, or allow them with a reason:\n{}",
        unexplained.len(),
        unexplained.join("\n")
    );
    let either = |t: Option<TypeId>| t.map_or_else(|| "any system".to_string(), system);
    let stale: Vec<String> = allowed
        .iter()
        .zip(&used)
        .filter(|(_, used)| !**used)
        .map(|(r, _)| match r.among.as_slice() {
            [] => format!("{} and {}: {}", either(r.a), either(r.b), r.why),
            sets => format!("any two among {sets:?}: {}", r.why),
        })
        .collect();
    assert!(stale.is_empty(), "allowed pairs of {game}'s that no longer conflict, to take off the list: {stale:#?}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Resource, Default)]
    struct Tally(u32);

    #[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
    struct Outer;

    #[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
    struct Inner;

    fn count_one(mut tally: ResMut<Tally>) {
        tally.0 += 1;
    }

    fn count_two(mut tally: ResMut<Tally>) {
        tally.0 += 2;
    }

    /// An app whose two systems both write a `Tally` unordered, `count_one`
    /// in `Inner`, a set inside `Outer`, and `count_two` in `Outer` when
    /// `both_inside`, otherwise in no set.
    fn checked(both_inside: bool) {
        let mut app = App::new();
        app.init_resource::<Tally>().configure_sets(Update, Inner.in_set(Outer)).add_systems(Update, count_one.in_set(Inner));
        if both_inside {
            app.add_systems(Update, count_two.in_set(Outer));
        } else {
            app.add_systems(Update, count_two);
        }
        let ours = added(&mut app, &mut App::new());
        let world = app.world();
        let tally = data([("Tally", world.components().component_id::<Tally>())]);
        let allowed = [Allowed::among(vec![Outer.intern()], on(&tally, &["Tally"]), "a test's two counters")];
        assert_ordered(world, "a test", &ours, &allowed, &[("count_one", system_type(count_one)), ("count_two", system_type(count_two))], &tally);
    }

    #[test]
    fn an_allowance_among_sets_admits_a_pair_inside_them() {
        checked(true);
    }

    #[test]
    #[should_panic(expected = "1 unordered conflicting pairs of a test's")]
    fn an_allowance_among_sets_admits_no_pair_outside_them() {
        checked(false);
    }
}
