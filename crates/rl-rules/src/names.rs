//! Where a content file's names are looked up.
//!
//! A definition in RON names other content: an ability names the stat it
//! spends and the status it requires, a monster names the damage kind it
//! deals and the faction it fights for, an item names the slot it is worn
//! in. Every such name is resolved once, at load, so nothing compares
//! strings during play and a typo fails at startup saying what was wrong.
//!
//! [`Names`] borrows whichever registries a game has, the engine's and its
//! own alike, and every loader reads through it. A game's own definition
//! type writes a named field as a [`NameRef`], loads its file with
//! [`Names::load`], and holds ids from then on: no validation pass, no
//! second lookup at spawn, and no string that could still be wrong.

use std::any::TypeId;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;

use rl_core::Id;
use ron::extensions::Extensions;
use ron::options::Options;
use ron::value::RawValue;
use serde::de::{DeserializeOwned, Error as _};
use serde::{Deserialize, Deserializer};

use crate::affix::{TagDef, TagId};
use crate::content::{ContentError, Named, Registry};
use crate::damage::{DamageKind, DamageKindId};
use crate::equip::{SlotDef, SlotId};
use crate::stats::{StatDef, StatId};
use crate::status::{StatusDef, StatusId};

/// A registry, as far as looking a name up in it goes.
trait Table {
    /// Every name and its raw id.
    fn entries(&self) -> Vec<(String, u32)>;
    /// The raw id of `name`.
    fn raw(&self, name: &str) -> Option<u32>;
}

impl<T> Table for Registry<T> {
    fn entries(&self) -> Vec<(String, u32)> {
        (0..self.len() as u32).map(|raw| (self.name(Id::from_raw(raw)).to_string(), raw)).collect()
    }

    fn raw(&self, name: &str) -> Option<u32> {
        self.id(name).map(Id::raw)
    }
}

/// One registry `Names` can look in, and what its entries are called.
#[derive(Clone, Copy)]
struct Shelf<'a> {
    kind: TypeId,
    what: &'static str,
    table: &'a dyn Table,
}

/// The registries a content file's names resolve against, borrowed for one
/// load.
///
/// Any may be left out. A name looked up in a registry that was never given
/// is reported as unknown, and says so, rather than panicking: a game with
/// no statuses loads abilities that name none, and one that names a status
/// anyway is told there is nothing to find it in.
///
/// Loads chain, because content names content: statuses name stats, and
/// abilities name statuses.
///
/// ```
/// use rl_rules::{DamageKind, Names, Registry, StatDef, ability, status};
///
/// let stats = Registry::from_defs(vec![StatDef::new("mana", 20)]).unwrap();
/// let kinds = Registry::from_defs(vec![DamageKind::new("fire")]).unwrap();
/// let names = Names::new().stats(&stats).damage_kinds(&kinds);
///
/// let statuses = status::load(r#"[(name: "burning", ticks: Some(("fire", 2)))]"#, &names).unwrap();
/// let names = names.statuses(&statuses);
/// let abilities = ability::load(r#"[(name: "ignite", mode: Adjacent, costs: [Pool("mana", 4)])]"#, &names).unwrap();
///
/// assert_eq!(statuses.get(statuses.expect("burning")).tick_damage, Some((kinds.expect("fire"), 2)));
/// assert_eq!(abilities.len(), 1);
/// assert!(names.tag("powder").is_err(), "no tags were given");
/// ```
#[derive(Clone, Default)]
pub struct Names<'a> {
    shelves: Vec<Shelf<'a>>,
}

impl<'a> Names<'a> {
    /// Nothing to look anything up in.
    pub fn new() -> Self {
        Self::default()
    }

    /// Any registry, the game's own included, with what one of its entries
    /// is called in a message: `with("item", &items)`.
    ///
    /// A second registry of the same type replaces the first.
    pub fn with<T: 'static>(mut self, what: &'static str, registry: &'a Registry<T>) -> Self {
        let kind = TypeId::of::<T>();
        self.shelves.retain(|s| s.kind != kind);
        self.shelves.push(Shelf { kind, what, table: registry });
        self
    }

    /// Stats, for a cost, a requirement or a modifier.
    pub fn stats(self, stats: &'a Registry<StatDef>) -> Self {
        self.with("stat", stats)
    }

    /// Statuses, for a requirement or an effect that inflicts one.
    pub fn statuses(self, statuses: &'a Registry<StatusDef>) -> Self {
        self.with("status", statuses)
    }

    /// Item tags, for an item cost, a requirement or an affix.
    pub fn tags(self, tags: &'a Registry<TagDef>) -> Self {
        self.with("tag", tags)
    }

    /// Equipment slots, for a requirement.
    pub fn slots(self, slots: &'a Registry<SlotDef>) -> Self {
        self.with("slot", slots)
    }

    /// Damage kinds, for a tick, a strike or an effect that deals one.
    pub fn damage_kinds(self, damage_kinds: &'a Registry<DamageKind>) -> Self {
        self.with("damage kind", damage_kinds)
    }

    /// The `T` named `name`, or why there is none.
    pub fn id<T: 'static>(&self, name: &str) -> Result<Id<T>, String> {
        self.find(TypeId::of::<T>(), short_name::<T>(), name).map(Id::from_raw)
    }

    /// The stat named `name`, or why there is none.
    pub fn stat(&self, name: &str) -> Result<StatId, String> {
        self.find(TypeId::of::<StatDef>(), "stat", name).map(Id::from_raw)
    }

    /// The status named `name`, or why there is none.
    pub fn status(&self, name: &str) -> Result<StatusId, String> {
        self.find(TypeId::of::<StatusDef>(), "status", name).map(Id::from_raw)
    }

    /// The tag named `name`, or why there is none.
    pub fn tag(&self, name: &str) -> Result<TagId, String> {
        self.find(TypeId::of::<TagDef>(), "tag", name).map(Id::from_raw)
    }

    /// The slot named `name`, or why there is none.
    pub fn slot(&self, name: &str) -> Result<SlotId, String> {
        self.find(TypeId::of::<SlotDef>(), "slot", name).map(Id::from_raw)
    }

    /// The damage kind named `name`, or why there is none.
    pub fn damage_kind(&self, name: &str) -> Result<DamageKindId, String> {
        self.find(TypeId::of::<DamageKind>(), "damage kind", name).map(Id::from_raw)
    }

    /// Loads a registry of a game's own definitions from RON, resolving every
    /// [`NameRef`] in them through these names.
    ///
    /// Reports every unknown name in the file at once, each prefixed with the
    /// name of the definition it is in, and every entry that does not parse.
    /// RON's `implicit_some` is on, so an optional field is written bare.
    pub fn load<T: DeserializeOwned + Named>(&self, text: &str) -> Result<Registry<T>, ContentError> {
        let options = Options::default().with_default_extension(Extensions::IMPLICIT_SOME);
        let entries: Vec<Box<RawValue>> = options.from_str(text).map_err(|e| ContentError::Parse(e.to_string()))?;
        let _scope = Scope::enter(self);
        let mut defs = Vec::new();
        let mut errors = Vec::new();
        for (i, entry) in entries.iter().enumerate() {
            let parsed = options.from_str::<T>(entry.get_ron());
            let unknown = Scope::take_unknown();
            match parsed {
                Ok(def) => {
                    errors.extend(unknown.into_iter().map(|e| format!("{}: {e}", def.name())));
                    defs.push(def);
                }
                Err(e) => errors.push(format!("entry {}: {e}", i + 1)),
            }
        }
        if !errors.is_empty() {
            return Err(ContentError::Invalid(errors));
        }
        Registry::from_defs(defs)
    }

    fn find(&self, kind: TypeId, what: &str, name: &str) -> Result<u32, String> {
        match self.shelves.iter().find(|s| s.kind == kind) {
            Some(shelf) => shelf.table.raw(name).ok_or_else(|| unknown(shelf.what, name)),
            None => Err(format!("unknown {what} {name:?}: no {what} registry was given to look it up in")),
        }
    }
}

fn unknown(what: &str, name: &str) -> String {
    format!("unknown {what} {name:?}")
}

/// `T`'s name without its path, for a message about a registry of a type
/// no label was given for.
fn short_name<T>() -> &'static str {
    std::any::type_name::<T>().rsplit("::").next().unwrap_or("entry")
}

/// A name in a content file, and the id of the `T` it names once the file
/// is loaded.
///
/// Written as a plain string in RON: `kind: "bite"`. Read only through
/// [`Names::load`], which knows what registries the names are in, so a
/// `NameRef` that exists names something that exists. Anywhere else it refuses
/// to deserialize rather than hold a name nobody checked.
pub struct NameRef<T>(Id<T>);

impl<T> NameRef<T> {
    /// The id it names.
    pub fn id(self) -> Id<T> {
        self.0
    }
}

impl<T> From<Id<T>> for NameRef<T> {
    fn from(id: Id<T>) -> Self {
        Self(id)
    }
}

impl<T> Clone for NameRef<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for NameRef<T> {}
impl<T> PartialEq for NameRef<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl<T> Eq for NameRef<T> {}
impl<T> fmt::Debug for NameRef<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<'de, T: 'static> Deserialize<'de> for NameRef<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        match Scope::resolve::<T>(&name) {
            None => Err(D::Error::custom(format!(
                "{name:?} names a `{}`, and a name is read through `Names::load`, which knows where to look it up",
                short_name::<T>()
            ))),
            Some(Ok(raw)) => Ok(NameRef(Id::from_raw(raw))),
            // The load this is part of fails and reports it, so the
            // placeholder is never read.
            Some(Err(())) => Ok(NameRef(Id::from_raw(0))),
        }
    }
}

/// The names a load is reading against, as the deserializer sees them.
///
/// Owned copies of the name tables rather than the borrowed registries,
/// because serde gives a `Deserialize` impl no way to be handed context, and
/// the only place to put it is a thread-local, which cannot hold a borrow.
/// Loads nest, so it is a stack.
struct Scope {
    tables: Vec<(TypeId, &'static str, BTreeMap<String, u32>)>,
    unknown: Vec<String>,
}

thread_local! {
    static SCOPES: RefCell<Vec<Scope>> = const { RefCell::new(Vec::new()) };
}

impl Scope {
    /// Opens a scope over `names` for as long as the guard lives.
    fn enter(names: &Names<'_>) -> ScopeGuard {
        let tables = names.shelves.iter().map(|s| (s.kind, s.what, s.table.entries().into_iter().collect())).collect();
        SCOPES.with(|scopes| scopes.borrow_mut().push(Scope { tables, unknown: Vec::new() }));
        ScopeGuard
    }

    /// The raw id of the `T` named `name` in the innermost scope, `Err` when
    /// it names nothing (recorded for the load to report), and `None` when
    /// no load is under way.
    fn resolve<T: 'static>(name: &str) -> Option<Result<u32, ()>> {
        SCOPES.with(|scopes| {
            let mut scopes = scopes.borrow_mut();
            let scope = scopes.last_mut()?;
            let kind = TypeId::of::<T>();
            let found = match scope.tables.iter().find(|(k, _, _)| *k == kind) {
                Some((_, what, table)) => table.get(name).copied().ok_or_else(|| unknown(what, name)),
                None => {
                    let what = short_name::<T>();
                    Err(format!("unknown {what} {name:?}: no {what} registry was given to look it up in"))
                }
            };
            Some(found.map_err(|e| scope.unknown.push(e)))
        })
    }

    /// The unknown names recorded since the last call.
    fn take_unknown() -> Vec<String> {
        SCOPES.with(|scopes| scopes.borrow_mut().last_mut().map(|s| std::mem::take(&mut s.unknown)).unwrap_or_default())
    }
}

/// Closes the innermost scope when dropped, a panic included.
struct ScopeGuard;

impl Drop for ScopeGuard {
    fn drop(&mut self) {
        SCOPES.with(|scopes| {
            scopes.borrow_mut().pop();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::faction::FactionDef;

    #[test]
    fn a_name_resolves_to_the_id_its_registry_issued() {
        let stats = Registry::from_defs(vec![StatDef::new("mana", 10), StatDef::new("nerve", 5)]).unwrap();
        let names = Names::new().stats(&stats);
        assert_eq!(names.stat("nerve"), Ok(stats.expect("nerve")));
        assert_eq!(names.stat("wisdom"), Err("unknown stat \"wisdom\"".to_string()));
    }

    #[test]
    fn a_registry_that_was_never_given_is_named_in_the_failure() {
        let names = Names::new();
        let why = names.status("burning").expect_err("there are no statuses");
        assert!(why.starts_with("unknown status \"burning\""), "{why}");
        assert!(why.contains("no status registry was given"), "{why}");
    }

    /// A game's own definition, naming the engine's registries and its own.
    #[derive(Debug, Deserialize)]
    struct Beast {
        name: String,
        bite: NameRef<DamageKind>,
        side: NameRef<FactionDef>,
        #[serde(default)]
        drops: Vec<(NameRef<Loot>, u32)>,
        #[serde(default)]
        fears: Option<NameRef<DamageKind>>,
    }

    impl Named for Beast {
        fn name(&self) -> &str {
            &self.name
        }
    }

    #[derive(Debug, Deserialize)]
    struct Loot {
        name: String,
    }

    impl Named for Loot {
        fn name(&self) -> &str {
            &self.name
        }
    }

    struct Vocabulary {
        kinds: Registry<DamageKind>,
        sides: Registry<FactionDef>,
        loot: Registry<Loot>,
    }

    impl Vocabulary {
        fn new() -> Self {
            Self {
                kinds: Registry::from_defs(vec![DamageKind::new("bite"), DamageKind::new("fire")]).unwrap(),
                sides: Registry::from_defs(vec![FactionDef { name: "beasts".into() }]).unwrap(),
                loot: Registry::from_ron_str(r#"[(name: "hide"), (name: "tooth")]"#).unwrap(),
            }
        }

        fn names(&self) -> Names<'_> {
            Names::new().damage_kinds(&self.kinds).with("faction", &self.sides).with("item", &self.loot)
        }
    }

    #[test]
    fn a_games_own_definitions_hold_ids_for_every_name_they_were_written_with() {
        let v = Vocabulary::new();
        let beasts: Registry<Beast> = v
            .names()
            .load(
                r#"#![enable(implicit_some)]
                [
                    (name: "wolf", bite: "bite", side: "beasts", drops: [("hide", 1), ("tooth", 2)]),
                    (name: "salamander", bite: "fire", side: "beasts", fears: "bite"),
                ]"#,
            )
            .expect("the file loads");
        let wolf = beasts.get(beasts.expect("wolf"));
        assert_eq!(wolf.bite.id(), v.kinds.expect("bite"));
        assert_eq!(wolf.side.id(), v.sides.expect("beasts"));
        assert_eq!(wolf.drops.iter().map(|(r, n)| (r.id(), *n)).collect::<Vec<_>>(), vec![(v.loot.expect("hide"), 1), (v.loot.expect("tooth"), 2)]);
        assert_eq!(beasts.get(beasts.expect("salamander")).fears.map(NameRef::id), Some(v.kinds.expect("bite")), "an optional name, written bare");
    }

    #[test]
    fn every_unknown_name_is_reported_at_once_under_the_definition_it_is_in() {
        let v = Vocabulary::new();
        let bad = r#"[
            (name: "wolf", bite: "claw", side: "beasts", drops: [("pelt", 1)]),
            (name: "crab", bite: "bite", side: "crabs"),
            (name: "fine", bite: "bite", side: "beasts"),
            (name: "broken", bite: 3, side: "beasts"),
        ]"#;
        let Err(ContentError::Invalid(errs)) = v.names().load::<Beast>(bad) else { panic!("a file of unknown names loaded") };
        assert_eq!(errs.len(), 4, "{errs:#?}");
        assert!(errs.contains(&"wolf: unknown damage kind \"claw\"".to_string()), "{errs:#?}");
        assert!(errs.contains(&"wolf: unknown item \"pelt\"".to_string()), "{errs:#?}");
        assert!(errs.contains(&"crab: unknown faction \"crabs\"".to_string()), "{errs:#?}");
        assert!(errs.iter().any(|e| e.starts_with("entry 4: ")), "an entry that does not parse is named by position: {errs:#?}");
    }

    #[test]
    fn a_name_read_outside_a_load_is_refused_rather_than_trusted() {
        let read: Result<NameRef<DamageKind>, _> = ron::from_str("\"bite\"");
        assert!(read.is_err_and(|e| e.to_string().contains("Names::load")));
        let generic = Names::new().id::<Loot>("hide").expect_err("no loot registry");
        assert!(generic.contains("unknown Loot \"hide\""), "{generic}");
    }
}
