//! What the adventurer knows of a kind's traits.
//!
//! A monster's plain traits, its look, its speed, whether it flies, are
//! known the moment its kind is seen at all: [`TraitDef::trick`](crate::traits::TraitDef::trick), written
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
//! fire or of slowness, conjured blades, a raised body; and for what a
//! `triggers:` trait grants, what the trigger's effect did rather than its
//! moment coming round, since a touch whose chance fails fires all the
//! same. A status it puts on is learned off the engine's own
//! [`StatusEvent`] in [`learn_from_statuses`], a venomous bite, a
//! paralysing touch, a wraith's withering touch; a theft where the thing is
//! taken, in [`crate::tricks::steal`]; and only a spore-burst, which lands
//! whenever it fires, off the actor triggers' own [`Fired`] in
//! [`learn_from_triggers`]. [`fire-trail`](crate::tricks::FireTrail) and
//! [`draining`](crate::tricks::Drains) are the bestiary's own game
//! systems; [`learn_fire_trail`] and [`learn_draining`] read the same
//! messages those systems already do, rather than threading a new message
//! through them. [`splitting`](crate::tricks::Splits) is learned where the
//! split itself is made, since whether a blow splits anything is decided
//! there and nowhere a message says.
//!
//! `reflecting` is learned off the engine's own `Reflected`, written by
//! [`atglyph::ecs::Bystanders::reflect`] each time it turns a
//! landing back: [`learn_reflecting`] reads it at the reflecting body's
//! own cell exactly as [`learn_draining`] reads the drainer's.

use std::collections::{BTreeMap, BTreeSet};

use atglyph::prelude::all::*;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::bestiary::{Bestiary, Kind, KindTag};
use crate::tricks::{Drains, FireTrail};

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
pub fn annotate_inspect(view: Option<ResMut<InspectView>>, kinds: Query<&Kind>, bestiary: Res<Bestiary>, lore: Res<MonsterLore>) {
    let Some(mut view) = view else { return };
    let Some(entity) = view.subject.as_ref().map(|s| s.entity) else { return };
    let Ok(kind) = kinds.get(entity) else { return };
    let def = bestiary.defs.get(kind.0);
    for (name, says, trick) in &def.folded.says {
        let text = if *trick && !lore.has_shown(&def.name, name) { "?".to_string() } else { says.clone() };
        let tone = if *trick && !lore.has_shown(&def.name, name) { Tones::MUTED } else { Tones::TEXT };
        view.facets.push(Facet::new("trait", text).toned(tone));
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

/// Learns every trick landed through the engine's own actor triggers that
/// does what it does whenever its moment comes, by
/// [`crate::bestiary::MonsterDef::trick_moments`]: a burst of spores where
/// something died, at [`Fired::at`], the cell it happened at.
///
/// Read through [`KindTag`] rather than [`Kind`], since a `died` moment
/// fires a pass later on the engine's remnant of the monster, which the
/// tag goes to and the saved kind does not.
///
/// In `TurnSet::React`, after combat and the engine's own actor triggers
/// have reported the pass's [`Fired`].
pub fn learn_from_triggers(
    mut fired: MessageReader<Fired>,
    kinds: Query<&KindTag>,
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

/// Learns every trick whose triggers put a status on what they touch, by
/// [`crate::bestiary::MonsterDef::trick_statuses`], the moment that status
/// takes hold where the adventurer can see: a bite's poison, a touch that
/// freezes, a touch that saps strength. Off the status taking hold rather
/// than the touch landing, since a touch's chance may fail, and a fourth
/// sapping of strength is refused.
///
/// The engine's own [`StatusEvent`] says what took hold and on whom, and
/// the [`ApplyStatus`] that asked for it says by whom; a status taking hold
/// is paired with an ask of this pass for the same status on the same
/// body, by one of the kind.
///
/// In `TurnSet::React`, after the pass's afflictions are resolved.
pub fn learn_from_statuses(
    mut asked: MessageReader<ApplyStatus>,
    mut took: MessageReader<StatusEvent>,
    kinds: Query<&Kind>,
    bodies: Query<&Position>,
    bestiary: Res<Bestiary>,
    players: Query<&Viewshed, With<Player>>,
    mut lore: ResMut<MonsterLore>,
) {
    let asks: Vec<(Entity, StatusId, Entity)> = asked.read().filter_map(|a| a.by.map(|by| (a.target, a.status, by))).collect();
    for ev in took.read() {
        let (StatusEvent::Applied { target, status } | StatusEvent::Renewed { target, status }) = *ev else { continue };
        let Ok(at) = bodies.get(target) else { continue };
        if !seen(&players, at.0) {
            continue;
        }
        for (_, _, by) in asks.iter().filter(|(t, s, _)| *t == target && *s == status) {
            let Ok(kind) = kinds.get(*by) else { continue };
            let def = bestiary.defs.get(kind.0);
            for (_, trait_name) in def.trick_statuses.iter().filter(|(s, _)| *s == status) {
                lore.show(&def.name, trait_name);
            }
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

/// Learns `reflecting` the moment a landing turned back is seen: the
/// engine's own `Reflected`, at the reflecting body's own cell.
///
/// In `TurnSet::React`.
pub fn learn_reflecting(
    mut reflected: MessageReader<Reflected>,
    reflecting: Query<(&Kind, &Position), With<Reflects>>,
    players: Query<&Viewshed, With<Player>>,
    bestiary: Res<Bestiary>,
    mut lore: ResMut<MonsterLore>,
) {
    for r in reflected.read() {
        let Ok((kind, pos)) = reflecting.get(r.by) else { continue };
        if seen(&players, pos.0) {
            lore.show(&bestiary.defs.get(kind.0).name, "reflecting");
        }
    }
}

#[cfg(test)]
mod tests {
    use atglyph::primitives::RunSeed;
    use atglyph::rules::Hit;

    use super::*;
    use crate::dungeon::map_of;
    use crate::testing::{adventurer, arena_with, arrive, continued, monster, said, settle, wall_off};

    /// Opens the look cursor on `at` and returns the words of every facet
    /// it shows, in order, closing the cursor again after.
    fn facets_at(app: &mut App, at: Point) -> Vec<String> {
        let modal = atglyph::ui::view::inspect::inspect_modal(app.world().resource::<Modals>());
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

    /// A golem's `reflecting` reads "?" on the look panel until the
    /// adventurer has seen a landing turned back off it, and its own
    /// words once it has.
    #[test]
    fn a_reflected_landing_teaches_reflecting_once_seen() {
        let (mut app, _, here) = arena_with(4, InspectViewPlugin);
        app.add_systems(Update, annotate_inspect.in_set(ViewSet::Annotate));
        let golem = monster(&mut app, "golem", here.offset(1, 0));
        let before = facets_at(&mut app, here.offset(1, 0));
        assert!(before.contains(&"?".to_string()) && !before.iter().any(|s| s == "turns magic back"), "unshown yet: {before:?}");

        app.world_mut().write_message(Reflected { by: golem, onto: golem, source: atglyph::ecs::Source::Offer(golem) });
        settle(&mut app);

        let after = facets_at(&mut app, here.offset(1, 0));
        assert!(after.contains(&"turns magic back".to_string()) && !after.iter().any(|s| s == "?"), "shown now: {after:?}");
        assert!(app.world().resource::<MonsterLore>().has_shown("golem", "reflecting"));
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

    /// A ghoul's `hit` moment coming round in plain sight is not its
    /// touch freezing anyone: with nothing frozen, `paralysing` stays
    /// unknown, and the pass the ghoul's own paralysis takes hold on the
    /// adventurer, it is known.
    #[test]
    fn a_paralysing_touch_is_learned_once_the_paralysis_takes_hold_and_not_before() {
        use crate::testing::arena;
        let (mut app, me, here) = arena(6);
        let ghoul = monster(&mut app, "ghoul", here.offset(1, 0));
        app.world_mut().entity_mut(ghoul).remove::<Mind>();
        settle(&mut app);
        app.world_mut().write_message(Fired { on: ghoul, moment: TriggerKinds::HIT, by: Some(ghoul), at: here });
        settle(&mut app);
        assert!(!app.world().resource::<MonsterLore>().has_shown("ghoul", "paralysing"), "a touch that froze nothing shows nothing");
        let paralysed = app.world().resource::<Registries>().statuses.expect("paralysed");
        app.world_mut().write_message(ApplyStatus { target: me, status: paralysed, turns: 2, by: Some(ghoul), held_by: None });
        settle(&mut app);
        assert!(app.world().resource::<MonsterLore>().has_shown("ghoul", "paralysing"), "frozen in plain sight, so known");
    }

    /// A monkey's blow at an adventurer with nothing loose in their pack
    /// takes nothing and teaches nothing of `stealing`; one at a pack with
    /// something in it takes it, and the theft seen is the trick known.
    #[test]
    fn stealing_is_learned_where_a_thing_is_taken_and_not_from_an_empty_pack() {
        use crate::testing::{arena, make};
        let (mut app, me, here) = arena(7);
        let monkey = monster(&mut app, "monkey", here.offset(1, 0));
        settle(&mut app);
        let bag: Vec<Entity> = app.world().get::<Inventory>(me).map(|b| b.items.clone()).unwrap_or_default();
        app.world_mut().get_mut::<Inventory>(me).expect("a pack").items.clear();
        assert!(!crate::tricks::steal(app.world_mut(), monkey, me), "nothing loose to take");
        assert!(!app.world().resource::<MonsterLore>().has_shown("monkey", "stealing"), "and nothing learned of it");
        let thing = make(&mut app, "dagger");
        app.world_mut().get_mut::<Inventory>(me).expect("a pack").items.extend(bag.into_iter().chain([thing]));
        assert!(crate::tricks::steal(app.world_mut(), monkey, me), "something taken");
        assert!(app.world().resource::<MonsterLore>().has_shown("monkey", "stealing"), "taken in plain sight, so known");
    }

    /// A myconid's `spore-burst` lands a pass after it dies, on the
    /// engine's remnant of it rather than on the myconid: seen bursting
    /// where it fell, it is known.
    #[test]
    fn a_myconid_seen_bursting_where_it_fell_teaches_spore_burst() {
        use crate::testing::{arena, kill};
        let (mut app, _, here) = arena(8);
        let myconid = monster(&mut app, "myconid", here.offset(1, 0));
        settle(&mut app);
        kill(&mut app, myconid, None);
        settle(&mut app);
        assert!(app.world().resource::<MonsterLore>().has_shown("myconid", "spore-burst"));
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
