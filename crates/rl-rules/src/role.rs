//! Roles: which monsters fit which part in a prefab.
//!
//! A prefab's monster slot may ask for a role rather than a monster, so a
//! room keeps its shape on every floor while what stands in it changes: a
//! doorway held by something heavy is held by the heaviest thing the
//! floor has. A role is only a name and its members. Where and how often
//! each member turns up is the spawn table's, written once, so a role
//! draw is the game's own spawn table restricted to the role's members,
//! at the slot's band, with the rows' own weights, falling back the way a
//! tagged loot draw does when no member is found that deep.
//!
//! A roles file is a map from a role's name to its members' names, and
//! [`load`] resolves every one of them at once, reporting every problem
//! in the file together.

use std::collections::BTreeMap;

use rand::Rng;
use rl_core::Id;

use crate::content::{BandedTable, ContentError, Named, Registry};
use crate::names::Names;

/// A part in a prefab and the monsters that fit it.
pub struct RoleDef<M> {
    /// The name a prefab's slot asks for.
    pub name: String,
    /// Who fits, in the order the file names them.
    pub members: Vec<Id<M>>,
}

/// A role, by id.
pub type RoleId<M> = Id<RoleDef<M>>;

impl<M> Named for RoleDef<M> {
    fn name(&self) -> &str {
        &self.name
    }
}

impl<M> std::fmt::Debug for RoleDef<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RoleDef").field("name", &self.name).field("members", &self.members).finish()
    }
}

impl<M> RoleDef<M> {
    /// Whether `id` fits the role.
    pub fn fits(&self, id: Id<M>) -> bool {
        self.members.contains(&id)
    }
}

/// Loads a roles file, `{ "role": ["monster", ..], .. }`, resolving every
/// member through `names`, whose registry of `M` is the game's monsters.
///
/// Refuses a role with no members, a name that is no monster, and a
/// member named twice in one role, every one of them at once. Roles are
/// numbered in name order, the order the map is read in.
pub fn load<M: 'static>(text: &str, names: &Names<'_>) -> Result<Registry<RoleDef<M>>, ContentError> {
    let authored: BTreeMap<String, Vec<String>> = ron::from_str(text).map_err(|e| ContentError::Parse(e.to_string()))?;
    let mut errors = Vec::new();
    let mut roles = Vec::new();
    for (name, said) in authored {
        if said.is_empty() {
            errors.push(format!("{name}: a role with no members is a slot nothing can stand in"));
        }
        let mut members = Vec::new();
        for member in &said {
            match names.id::<M>(member) {
                Ok(id) if members.contains(&id) => errors.push(format!("{name}: {member:?} is named twice")),
                Ok(id) => members.push(id),
                Err(e) => errors.push(format!("{name}: {e}")),
            }
        }
        roles.push(RoleDef { name, members });
    }
    if !errors.is_empty() {
        return Err(ContentError::Invalid(errors));
    }
    Registry::from_defs(roles)
}

/// The band a draw for `role` at `band` is made at: `band` when a member
/// with a weighted row applies there, else the nearest band that has one,
/// shallower first. `None` when no member has a weighted row at all.
pub fn band_for<M>(table: &BandedTable<Id<M>>, role: &RoleDef<M>, band: i32) -> Option<i32> {
    table.band_where(band, |id| role.fits(*id))
}

/// Draws a member of `role` from `table` at `band`, or at the band
/// [`band_for`] falls back to, by the rows' own weights.
pub fn draw<M>(table: &BandedTable<Id<M>>, role: &RoleDef<M>, band: i32, rng: &mut impl Rng) -> Option<Id<M>> {
    let at = band_for(table, role, band)?;
    table.pick_where(at, |id| role.fits(*id), rng).map(|e| e.item)
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;

    use super::*;
    use crate::content::BandedEntry;

    /// A test monster: only a name.
    struct Beast(&'static str);
    impl Named for Beast {
        fn name(&self) -> &str {
            self.0
        }
    }

    fn beasts() -> Registry<Beast> {
        Registry::from_defs(vec![Beast("rat"), Beast("crab"), Beast("heavy"), Beast("moth")]).unwrap()
    }

    fn table(b: &Registry<Beast>) -> BandedTable<Id<Beast>> {
        BandedTable::new(vec![
            BandedEntry::new(b.expect("rat")).bands(1, 10).weight(5),
            BandedEntry::new(b.expect("crab")).bands(2, 7).weight(3),
            BandedEntry::new(b.expect("heavy")).bands(3, 8).weight(2),
        ])
    }

    #[test]
    fn a_roles_file_names_each_role_and_the_monsters_that_fit_it() {
        let b = beasts();
        let roles: Registry<RoleDef<Beast>> = load(r#"{ "brute": ["heavy", "crab"], "swarm": ["rat"] }"#, &Names::new().with("monster", &b)).unwrap();
        let brute = roles.get(roles.expect("brute"));
        assert_eq!(brute.members, vec![b.expect("heavy"), b.expect("crab")]);
        assert!(brute.fits(b.expect("crab")) && !brute.fits(b.expect("rat")));
    }

    #[test]
    fn a_roles_file_refuses_every_empty_role_unknown_monster_and_repeated_member_at_once() {
        let b = beasts();
        let err = load::<Beast>(r#"{ "empty": [], "typo": ["hevy"], "twice": ["rat", "rat"] }"#, &Names::new().with("monster", &b)).unwrap_err().to_string();
        for said in ["empty", "hevy", "twice"] {
            assert!(err.contains(said), "{said:?} missing from: {err}");
        }
    }

    #[test]
    fn a_role_draw_only_ever_returns_a_member_of_the_role() {
        let b = beasts();
        let roles: Registry<RoleDef<Beast>> = load(r#"{ "brute": ["heavy", "crab"] }"#, &Names::new().with("monster", &b)).unwrap();
        let (brute, table) = (roles.get(roles.expect("brute")), table(&b));
        for band in 1..=12 {
            for s in 0..100 {
                let got = draw(&table, brute, band, &mut rand::rngs::StdRng::seed_from_u64(s)).unwrap();
                assert!(brute.fits(got), "band {band}, seed {s}: {}", b.name(got));
            }
        }
    }

    #[test]
    fn a_role_asked_past_the_deepest_row_draws_from_the_deepest_band() {
        let b = beasts();
        let roles: Registry<RoleDef<Beast>> = load(r#"{ "brute": ["heavy", "crab"] }"#, &Names::new().with("monster", &b)).unwrap();
        let brute = roles.get(roles.expect("brute"));
        assert_eq!(band_for(&table(&b), brute, 12), Some(8));
        assert_eq!(band_for(&table(&b), brute, 5), Some(5));
    }

    #[test]
    fn a_role_none_of_whose_members_has_a_spawn_row_draws_nothing() {
        let b = beasts();
        let roles: Registry<RoleDef<Beast>> = load(r#"{ "fliers": ["moth"] }"#, &Names::new().with("monster", &b)).unwrap();
        let fliers = roles.get(roles.expect("fliers"));
        assert_eq!(band_for(&table(&b), fliers, 5), None);
        assert_eq!(draw(&table(&b), fliers, 5, &mut rand::rngs::StdRng::seed_from_u64(0)), None);
    }
}
