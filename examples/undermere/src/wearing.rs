//! Worn things lend their traits.
//!
//! An item in `items.ron` lends traits from `traits.ron` to whoever wears
//! it, the same traits a monster is written as, so a ring of the
//! salamander and a salamander are fire-blooded for the same reason. The
//! [`Armory`](crate::armory::Armory) folds what each thing lends once, at
//! load, and puts as much of it as it can where the engine already reads
//! a worn thing: the armor and the resistances a thing lends join its own
//! `Armor` and `Resists`, which the engine's `Loadout` counts for its
//! wearer, and only while it is worn.
//!
//! The rest the engine cannot read off a worn thing, and it is the
//! thing's [`Lends`], which [`lend`] keeps its wearer in step with every
//! pass, through the engine's own seams:
//!
//! - a status is held by the thing, put on for life with the engine's
//!   hold, so the engine itself cures it the pass the thing comes off,
//!   however it came off;
//! - sight in the dark is the wearer's `DarkSight`, the better of its own
//!   and the best any thing it wears lends, given back as it was once
//!   nothing it wears lends any;
//! - a light is the thing's own `LightSource`, which the engine sheds from
//!   whoever carries it, lit while the thing is worn and put out when it
//!   is not.
//!
//! Undermere registers no stats, so nothing it lends goes through the
//! engine's `Bestows`, which folds a worn thing's changes to a stat:
//! armor, the one number a trait sets that a stat could have carried, is
//! the engine's `Armor` instead. What a worn thing cannot give at all, a
//! mind's wits, a pace, how far a body sees, notices and hears, when it
//! runs and what it knows how to do, is refused where the item is read.

use bevy::prelude::*;
use rl_engine::prelude::*;

use crate::bestiary::FOR_LIFE;
use crate::traits::Folded;

/// What a worn thing gives its wearer that the engine cannot read off the
/// thing itself, kept in step by [`lend`].
///
/// Put on the thing when it is made, from what it lends, folded; a thing
/// lending only armor and resistances carries none, since those are its
/// own `Armor` and `Resists`.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct Lends {
    /// Every status it holds on its wearer for as long as it is worn.
    pub statuses: Vec<StatusId>,
    /// Tiles its wearer sees with no light at all.
    pub dark_sight: Option<i32>,
    /// The light it sheds while it is worn.
    pub light: Option<LightSource>,
}

impl Lends {
    /// What `lent` gives a wearer that has to be put on the wearer, or
    /// `None` when it gives nothing of the kind.
    pub fn of(lent: &Folded) -> Option<Self> {
        let lends = Self { statuses: lent.statuses.clone(), dark_sight: lent.dark_sight, light: lent.light };
        (!lends.statuses.is_empty() || lends.dark_sight.is_some() || lends.light.is_some()).then_some(lends)
    }
}

/// A wearer's own sight in the dark, kept while something it wears lends
/// it more, so taking that off gives back what it had: `None` for a
/// wearer that sees nothing without light.
///
/// Kept on the wearer rather than read off its kind, since an adventurer
/// and a monster come by their own sight differently, and taken the first
/// time a thing lends any, before anything has been lent.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnSight(pub Option<i32>);

/// A wearer: what it wears, the statuses it holds, if it can hold any, and
/// its sight in the dark, its own and as it stands.
type Wearer = (Entity, &'static Equipped, Option<&'static Afflicted>, Option<&'static DarkSight>, Option<&'static OwnSight>);

/// Keeps every wearer in step with what the things it wears lend, and
/// every lending thing's light with whether it is worn.
///
/// Run every pass in `TurnSet::React`, deriving what each wearer should
/// have from what it wears now rather than answering each way a thing goes
/// on or comes off, as Foundry's dark sight is kept: worn, dropped,
/// displaced, taken by a goblin or restored from a save, the next pass
/// reads it as it stands. Idempotent: what already reads right is left
/// alone, so nothing is changed for the light or the sight to recast.
///
/// A status is asked for, through `Afflict`, only where no worn thing that
/// lends it holds it, and held by the thing that asks; what takes it off
/// again is the engine's own `end_unworn_holds`, the pass the thing is no
/// longer worn.
pub fn lend(mut commands: Commands, wearers: Query<Wearer>, lending: Query<(Entity, &Lends, Option<&LightSource>)>, mut afflict: MessageWriter<Afflict>) {
    let mut lit: Vec<Entity> = Vec::new();
    for (wearer, equipped, afflicted, sight, own) in &wearers {
        // Each lending thing once, though a two-hander fills two slots.
        let mut worn: Vec<(Entity, &Lends)> = Vec::new();
        for (_, item) in equipped.0.worn() {
            if let Ok((_, lends, _)) = lending.get(item)
                && !worn.iter().any(|(seen, _)| *seen == item)
            {
                worn.push((item, lends));
            }
        }
        // A wearer that holds no statuses at all is lent none.
        if let Some(afflicted) = afflicted {
            for (item, lends) in &worn {
                for status in &lends.statuses {
                    let held_by_what_it_wears = |by: u64| worn.iter().any(|(holder, _)| holder.to_bits() == by);
                    if !afflicted.iter().any(|s| s.id == *status && s.held_by.is_some_and(held_by_what_it_wears)) {
                        afflict.write(Afflict { target: wearer, status: *status, turns: FOR_LIFE, by: None, held_by: Some(*item) });
                    }
                }
            }
        }
        see_in_the_dark(&mut commands, wearer, worn.iter().filter_map(|(_, lends)| lends.dark_sight).max(), sight.map(|s| s.0), own.copied());
        lit.extend(worn.iter().filter(|(_, lends)| lends.light.is_some()).map(|(item, _)| *item));
    }
    for (item, lends, light) in &lending {
        let Some(lent) = lends.light else { continue };
        match (lit.contains(&item), light) {
            (true, Some(now)) if *now == lent => {}
            (true, _) => {
                commands.entity(item).insert(lent);
            }
            (false, Some(_)) => {
                commands.entity(item).remove::<LightSource>();
            }
            (false, None) => {}
        }
    }
}

/// Gives `wearer`, which sees `sight` in the dark now, the better of its
/// own sight and the `lent` best of what it wears, or gives back its own
/// once nothing it wears lends any. Its own is kept in an [`OwnSight`]
/// from the first time anything is lent until nothing is.
fn see_in_the_dark(commands: &mut Commands, wearer: Entity, lent: Option<i32>, sight: Option<i32>, own: Option<OwnSight>) {
    match (lent, own) {
        (Some(lent), _) => {
            let mine = own.map_or(sight, |o| o.0);
            let best = mine.map_or(lent, |m| m.max(lent));
            let mut e = commands.entity(wearer);
            if own.is_none() {
                e.insert(OwnSight(mine));
            }
            if sight != Some(best) {
                e.insert(DarkSight(best));
            }
        }
        (None, Some(OwnSight(mine))) => {
            let mut e = commands.entity(wearer);
            match mine {
                Some(cells) if sight != Some(cells) => {
                    e.insert(DarkSight(cells));
                }
                Some(_) => {}
                None => {
                    e.remove::<DarkSight>();
                }
            }
            e.remove::<OwnSight>();
        }
        (None, None) => {}
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;
    use bevy::ecs::world::CommandQueue;
    use rl_engine::rl_core::{Rect, RunSeed};

    use super::*;
    use crate::armory::Armory;
    use crate::content::Library;
    use crate::dungeon::map_of;
    use crate::testing::{arrive, clear_monsters, equip, headless, pass_turns, unequip};

    /// A ring of the salamander, which the skeleton has no use for yet:
    /// a fixture, as the content's own tests write theirs. It is worn on
    /// the off hand, since the skeleton has no finger to put it on either.
    const RING: &str = r#"#![enable(implicit_some)]
    [
        (name: "ring of the salamander", glyph: '=', color: (1.0, 0.45, 0.2), slot: "off hand",
         lends: ["fire-blooded", "regenerates", "sees in the dark", "ember glow"]),
    ]"#;

    /// The traits, as the game reads them.
    fn traits() -> Registry<crate::traits::TraitDef> {
        Library::load(&crate::content::effect_kinds(), &Moments::default()).traits
    }

    /// The ring, made exactly as the game makes a thing, by an armory of
    /// the ring alone, and put in `me`'s pack. That armory is the test's,
    /// not the run's: only what the ring lends is read here.
    fn ring(app: &mut App, me: Entity) -> Entity {
        let traits = traits();
        let registries = app.world().resource::<Registries>().clone();
        let defs = crate::armory::read(RING, &registries, &traits).expect("the ring reads");
        let world = app.world();
        let armory =
            Armory::new(defs, LootTable::default(), &registries, &traits, world.resource::<EffectKinds>(), world.resource::<Moments>()).expect("and is made");
        let mut queue = CommandQueue::default();
        let ring = crate::armory::spawn_item(&mut Commands::new(&mut queue, app.world()), &armory, armory.defs.expect("ring of the salamander"), &registries);
        queue.apply(app.world_mut());
        crate::testing::give(app, me, ring);
        ring
    }

    /// Everything the ring could lend, as it stands on `me` and the ring.
    #[derive(Debug, PartialEq)]
    struct Lent {
        /// Percent of fire and of cold taken off a blow on `me`.
        resists: (i32, i32),
        /// How long `regenerating` has left on `me`, and whether the ring
        /// holds it.
        regenerating: Option<(u32, bool)>,
        /// What `me` sees with no light.
        dark_sight: Option<i32>,
        /// The ring's light.
        light: Option<LightSource>,
    }

    fn lent(app: &mut App, me: Entity, ring: Entity) -> Lent {
        let registries = app.world().resource::<Registries>().clone();
        let (fire, cold) = (registries.damage_kinds.expect("fire"), registries.damage_kinds.expect("cold"));
        let resists = app.world_mut().run_system_once(move |loadout: Loadout| loadout.resistances(me)).expect("a loadout");
        let world = app.world();
        let regenerating = registries.statuses.expect("regenerating");
        Lent {
            resists: (resists.get(fire), resists.get(cold)),
            regenerating: world
                .get::<Afflicted>(me)
                .and_then(|a| a.iter().find(|s| s.id == regenerating).map(|s| (s.turns, s.held_by == Some(ring.to_bits())))),
            dark_sight: world.get::<DarkSight>(me).map(|d| d.0),
            light: world.get::<LightSource>(ring).copied(),
        }
    }

    /// Worn, the ring makes its wearer fire-blooded, regenerating for as
    /// long as it is on, able to see in the dark and lit by an ember
    /// glow; taken off, it takes every one of them back.
    #[test]
    fn a_ring_that_lends_a_trait_gives_it_while_worn_and_takes_it_back() {
        let mut app = headless(RunSeed(2));
        let me = arrive(&mut app, map_of(3));
        clear_monsters(&mut app);
        let ring = ring(&mut app, me);
        let before = lent(&mut app, me, ring);
        assert_eq!(before, Lent { resists: (0, 0), regenerating: None, dark_sight: None, light: None }, "in the pack, it lends nothing");

        equip(&mut app, me, ring);
        let traits = traits();
        let ember = traits.get(traits.expect("ember glow")).light.expect("a glow").source();
        assert_eq!(
            lent(&mut app, me, ring),
            Lent { resists: (100, -50), regenerating: Some((FOR_LIFE, true)), dark_sight: Some(6), light: Some(ember) },
            "worn, it lends all four"
        );
        pass_turns(&mut app, 3);
        assert_eq!(lent(&mut app, me, ring).regenerating, Some((FOR_LIFE, true)), "and what it holds does not run down");

        unequip(&mut app, me, ring);
        assert_eq!(lent(&mut app, me, ring), before, "taken off, it takes all of it back");
    }

    /// A status the ring holds on its wearer for life is on the
    /// character sheet by its name and what it does, with no count of
    /// turns, however many turns go by.
    #[test]
    fn a_status_held_for_life_shows_no_count_of_turns() {
        use rl_engine::rl_bevy::testing::{KeyScriptPlugin, press};
        let (width, height) = (60, 30);
        let mut app = headless(RunSeed(2));
        app.add_plugins((KeyScriptPlugin, SheetPanel::new(Rect::new(0, 0, width, height))));
        app.insert_resource(Terminal::new(width, height, Vec2::ONE));
        let me = arrive(&mut app, map_of(3));
        clear_monsters(&mut app);
        let ring = ring(&mut app, me);
        equip(&mut app, me, ring);
        pass_turns(&mut app, 5);
        press(&mut app, KeyCode::KeyC);
        let terminal = app.world().resource::<Terminal>();
        let rows: Vec<String> = (0..height).map(|y| (0..width).map(|x| terminal.get(x, y).map_or(' ', |c| c.glyph)).collect()).collect();
        let inside: Vec<&str> = rows.iter().map(|r| r.trim_start_matches("\u{2502} ").trim_end_matches('\u{2502}').trim_end()).collect();
        let held = inside.iter().find(|r| r.starts_with("regenerating")).unwrap_or_else(|| panic!("no regenerating on the sheet: {rows:#?}"));
        assert_eq!(*held, "regenerating   mends 1 a turn", "{rows:#?}");
    }
}
