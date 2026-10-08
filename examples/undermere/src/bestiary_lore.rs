//! What the adventurer knows of a kind's traits.
//!
//! A monster's plain traits, its look, its speed, whether it flies, are
//! known the moment its kind is seen at all: [`TraitDef::trick`], written
//! once in `traits.ron`, says which of a trait are not. [`MonsterLore`],
//! saved with the run as [`crate::lore::Lore`] is, holds for each kind the
//! tricks the adventurer has seen land: a bite that poisons someone, a
//! jelly that splits, a necromancer that raises a body. The look panel
//! reads it in [`annotate_inspect`], printing a trick's own words once it
//! has, and a "?" for each it has not, so the player knows there is more
//! without knowing what.
//!
//! A handful of engine messages already carry enough to tell when a
//! trick's effect landed somewhere the adventurer could see: an ability's
//! own [`AbilityEvent::Used`] for everything an `abilities:` trait grants,
//! fire breath, a bite that spits webs, a blink, a ward, a mend, a word of
//! fire or of slowness, conjured blades, a raised body; and the engine's
//! own actor triggers' [`Fired`] for everything a `triggers:` trait
//! grants, a venomous bite, a paralysing touch, a wraith's withering
//! touch, a spore-burst, a monkey's theft, each at the cell the trigger
//! answered at. [`splitting`](crate::tricks::Splits), [`fire-trail`
//! ](crate::tricks::FireTrail) and [`draining`](crate::tricks::Drains) are
//! the bestiary's own game systems from task 6b; [`learn_splitting`],
//! [`learn_fire_trail`] and [`learn_draining`] read the same messages
//! those systems already do, rather than threading a new message through
//! them.
//!
//! `reflecting` moves a landing's targets silently inside
//! [`rl_engine::rl_bevy::Bystanders::reflect`], before anything is fired,
//! so nothing here tells a reflected landing apart from an ordinary one;
//! left unseen until the engine grows a seam for it, and said so in task
//! 7's report.

use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;
use rl_engine::prelude::*;
use serde::{Deserialize, Serialize};

use crate::bestiary::{Bestiary, Kind};
use crate::tricks::{Drains, FireTrail, Splits};

/// What the adventurer knows of the run's monsters: which of each kind's
/// tricks they have seen land. A plain trait needs no entry here; it is
/// read straight off `traits.ron` by [`annotate_inspect`].
#[derive(Resource, Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MonsterLore {
    /// Each kind's own tricks shown so far, by the kind's name and the
    /// trick trait's.
    shown: BTreeMap<String, BTreeSet<String>>,
}

impl MonsterLore {
    /// Whether `kind` has shown `trick`.
    pub fn has_shown(&self, kind: &str, trick: &str) -> bool {
        self.shown.get(kind).is_some_and(|tricks| tricks.contains(trick))
    }

    /// Marks `trick` shown for `kind`. Returns whether it was not shown
    /// before.
    pub fn show(&mut self, kind: &str, trick: &str) -> bool {
        self.shown.entry(kind.to_string()).or_default().insert(trick.to_string())
    }
}

/// Whether anyone the adventurer plays reads `at` as in their sight right
/// now: the same check [`crate::allies::fade`] makes of a conjured
/// thing's going.
fn seen(players: &Query<&Viewshed, With<Player>>, at: Point) -> bool {
    players.iter().any(|v| v.can_see(at))
}

/// Puts a monster's traits on the look panel in their own words: every
/// plain one outright, and every trick only once [`MonsterLore`] has
/// shown it for its kind, a "?" until then, so the player knows there is
/// more without knowing what.
///
/// In `ViewSet::Annotate`, reading what `collect_inspect` put under the
/// cursor this frame.
pub fn annotate_inspect(view: Option<ResMut<InspectView>>, kinds: Query<&Kind>, bestiary: Res<Bestiary>, lore: Res<MonsterLore>, mut facets: ResMut<Facets>) {
    let Some(mut view) = view else { return };
    let Some(entity) = view.subject.as_ref().map(|s| s.entity) else { return };
    let Ok(kind) = kinds.get(entity) else { return };
    let def = bestiary.defs.get(kind.0);
    for (name, says, trick) in &def.folded.says {
        let text = if *trick && !lore.has_shown(&def.name, name) { "?".to_string() } else { says.clone() };
        let tone = if *trick && !lore.has_shown(&def.name, name) { Tones::MUTED } else { Tones::TEXT };
        view.facets.push(facets.facet("trait", text).toned(tone));
    }
}

/// Learns every trick an ability landed, by [`crate::bestiary::MonsterDef::trick_abilities`]:
/// everything a trait grants as an ability, at the same `aim` the ability
/// landed at.
///
/// In `TurnSet::React`.
pub fn learn_from_abilities(
    mut events: MessageReader<AbilityEvent>,
    kinds: Query<&Kind>,
    bestiary: Res<Bestiary>,
    players: Query<&Viewshed, With<Player>>,
    mut lore: ResMut<MonsterLore>,
) {
    for ev in events.read() {
        let AbilityEvent::Used { user, ability, aim, .. } = ev else { continue };
        let Ok(kind) = kinds.get(*user) else { continue };
        let def = bestiary.defs.get(kind.0);
        let Some((_, trait_name)) = def.trick_abilities.iter().find(|(a, _)| a == ability) else { continue };
        if seen(&players, *aim) {
            lore.show(&def.name, trait_name);
        }
    }
}

/// Learns every trick landed through the engine's own actor triggers, by
/// [`crate::bestiary::MonsterDef::trick_moments`]: a bite's poison, a
/// touch's chill or its sapped strength, a hand that steals, a burst of
/// spores where something died, each at [`Fired::at`], the cell it
/// happened at.
///
/// In `TurnSet::React`, after combat and the engine's own actor triggers
/// have reported the pass's [`Fired`].
pub fn learn_from_triggers(
    mut fired: MessageReader<Fired>,
    kinds: Query<&Kind>,
    bestiary: Res<Bestiary>,
    players: Query<&Viewshed, With<Player>>,
    mut lore: ResMut<MonsterLore>,
) {
    for f in fired.read() {
        let Ok(kind) = kinds.get(f.on) else { continue };
        let def = bestiary.defs.get(kind.0);
        for (moment, trait_name) in &def.trick_moments {
            if *moment == f.moment && seen(&players, f.at) {
                lore.show(&def.name, trait_name);
            }
        }
    }
}

/// Learns `splitting` the moment a blow a jelly lived through is seen
/// landing: the same `DamageDealt` and the same live-or-not check
/// [`crate::tricks::judge_splits`] makes, read again here rather than
/// threaded through it, so the two stay two small systems instead of one
/// doing both jobs.
///
/// In `TurnSet::React`.
pub fn learn_splitting(
    mut dealt: MessageReader<DamageDealt>,
    splitting: Query<(&Kind, &Position, &Health), With<Splits>>,
    players: Query<&Viewshed, With<Player>>,
    bestiary: Res<Bestiary>,
    mut lore: ResMut<MonsterLore>,
) {
    for hit in dealt.read().filter(|d| d.dealt > 0) {
        let Ok((kind, pos, health)) = splitting.get(hit.target) else { continue };
        if health.current > 0 && seen(&players, pos.0) {
            lore.show(&bestiary.defs.get(kind.0).name, "splitting");
        }
    }
}

/// Learns `fire-trail` the moment a step it left burning is seen: the
/// same `Stepped` [`crate::tricks::trail_fire`] reads, at the cell it
/// stepped off.
///
/// In `TurnSet::React`.
pub fn learn_fire_trail(
    mut stepped: MessageReader<Stepped>,
    trailing: Query<&Kind, With<FireTrail>>,
    players: Query<&Viewshed, With<Player>>,
    bestiary: Res<Bestiary>,
    mut lore: ResMut<MonsterLore>,
) {
    for step in stepped.read() {
        let Ok(kind) = trailing.get(step.actor) else { continue };
        if seen(&players, step.from) {
            lore.show(&bestiary.defs.get(kind.0).name, "fire-trail");
        }
    }
}

/// Learns `draining` the moment a landed blow is seen mending whoever
/// struck it: the self-heal [`crate::tricks::judge_draining`] writes, at
/// the drainer's own cell.
///
/// In `TurnSet::React`.
pub fn learn_draining(
    mut healed: MessageReader<Heal>,
    draining: Query<(&Kind, &Position), With<Drains>>,
    players: Query<&Viewshed, With<Player>>,
    bestiary: Res<Bestiary>,
    mut lore: ResMut<MonsterLore>,
) {
    for heal in healed.read().filter(|h| h.amount > 0 && h.by == Some(h.target)) {
        let Ok((kind, pos)) = draining.get(heal.target) else { continue };
        if seen(&players, pos.0) {
            lore.show(&bestiary.defs.get(kind.0).name, "draining");
        }
    }
}

#[cfg(test)]
mod tests {
    use rl_engine::rl_core::RunSeed;
    use rl_engine::rl_rules::Hit;

    use super::*;
    use crate::dungeon::map_of;
    use crate::testing::{adventurer, arena_with, arrive, continued, monster, said, settle, wall_off};

    /// Opens the look cursor on `at` and returns the words of every facet
    /// it shows, in order, closing the cursor again after.
    fn facets_at(app: &mut App, at: Point) -> Vec<String> {
        let modal = rl_engine::rl_ui::view::inspect::inspect_modal(app.world().resource::<Modals>());
        app.world_mut().resource_mut::<Modals>().open(modal);
        app.world_mut().resource_mut::<InspectView>().cursor = at;
        app.update();
        let facets = app.world().resource::<InspectView>().facets.iter().map(|f| f.text.clone()).collect();
        app.world_mut().resource_mut::<Modals>().close_one(modal);
        app.update();
        facets
    }

    /// A rat's plain traits, its wits, its running when hurt, its hunting
    /// in packs, are all named on the look panel the moment it is seen,
    /// with no trick's "?" among them: a rat carries none.
    #[test]
    fn a_monsters_plain_traits_are_named_on_sight() {
        let (mut app, _, here) = arena_with(1, InspectViewPlugin);
        app.add_systems(Update, annotate_inspect.in_set(ViewSet::Annotate));
        monster(&mut app, "rat", here.offset(1, 0));
        let said = facets_at(&mut app, here.offset(1, 0));
        assert!(said.contains(&"a beast".to_string()), "{said:?}");
        assert!(said.contains(&"runs when hurt".to_string()), "{said:?}");
        assert!(said.contains(&"hunts in packs".to_string()), "{said:?}");
        assert!(!said.iter().any(|s| s == "?"), "a rat carries no trick: {said:?}");
    }

    /// A pink jelly's `splitting` reads "?" on the look panel until the
    /// adventurer has seen it split, and its own words once it has.
    #[test]
    fn a_trick_reads_as_a_question_mark_until_shown_and_then_its_own_words() {
        let (mut app, _, here) = arena_with(2, InspectViewPlugin);
        app.add_systems(Update, annotate_inspect.in_set(ViewSet::Annotate));
        let jelly = monster(&mut app, "pink jelly", here.offset(1, 0));
        app.world_mut().entity_mut(jelly).insert(Health { current: 10, max: 30 });
        let before = facets_at(&mut app, here.offset(1, 0));
        assert!(before.contains(&"acts without thought".to_string()), "its plain trait is named at once: {before:?}");
        assert!(before.contains(&"?".to_string()) && !before.iter().any(|s| s == "splits when struck"), "unshown yet: {before:?}");

        let crush = app.world().resource::<Registries>().damage_kinds.expect("crush");
        app.world_mut().write_message(DamageEvent::new(jelly, Hit::from_source(None, crush, 4)));
        settle(&mut app);

        let after = facets_at(&mut app, here.offset(1, 0));
        assert!(after.contains(&"splits when struck".to_string()) && !after.iter().any(|s| s == "?"), "shown now: {after:?}");
        assert!(app.world().resource::<MonsterLore>().has_shown("pink jelly", "splitting"));
    }

    /// Review Focus 4: a jelly struck behind a wall, out of the
    /// adventurer's own sight, teaches nothing of `splitting`, and the
    /// log never names it either.
    #[test]
    fn a_trick_struck_out_of_sight_teaches_nothing_and_names_nothing_in_the_log() {
        let (mut app, _, here) = arena_with(3, InspectViewPlugin);
        wall_off(&mut app, &[here.offset(1, 0)]);
        let jelly = monster(&mut app, "pink jelly", here.offset(2, 0));
        app.world_mut().entity_mut(jelly).insert(Health { current: 10, max: 30 });
        settle(&mut app);
        let crush = app.world().resource::<Registries>().damage_kinds.expect("crush");
        app.world_mut().write_message(DamageEvent::new(jelly, Hit::from_source(None, crush, 4)));
        settle(&mut app);
        assert!(!app.world().resource::<MonsterLore>().has_shown("pink jelly", "splitting"), "never seen, so never known");
        let lines = said(&app);
        assert!(!lines.iter().any(|l| l.contains("splits into two")), "and never named in the log either: {lines:#?}");
    }

    /// What is known of a kind's tricks survives a save and a continued
    /// run, the same shape of life `Lore` already has.
    #[test]
    fn what_is_known_of_a_kinds_tricks_survives_a_save_and_continue() {
        let mut app = crate::testing::headless(RunSeed(4));
        adventurer(&mut app);
        app.world_mut().resource_mut::<MonsterLore>().show("pink jelly", "splitting");
        app.world().resource::<Saves>().delete(crate::save::SLOT).expect("the slot can be emptied");
        arrive(&mut app, map_of(1));
        let text = app.world().resource::<Saves>().load(crate::save::SLOT).expect("the slot reads").expect("the arrival wrote the run down");
        let carried = continued(&text);
        assert!(carried.world().resource::<MonsterLore>().has_shown("pink jelly", "splitting"));
    }
}
