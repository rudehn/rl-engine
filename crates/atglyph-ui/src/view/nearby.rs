//! What the player can see right now, as two lists, and which row of them
//! is picked out.
//!
//! The panel every roguelike grows: the actors in view with their health
//! and whose side they are on, and the things lying about. Every input is
//! already in the engine - the [`Viewshed`], the [`Position`]s, the
//! [`Health`], the faction matrix - which is why this is engine work. What
//! is not engine work is the weapon in a hand or the state of a mind, and
//! that arrives as a facet.
//!
//! The rows come in `InSight`'s order, the order the look and targeting
//! cursors step through, and the row picked out is the [`Focus`] those
//! cursors move. With nothing open, `browse` steps the same focus down
//! the rows on the cursors' "next" key, so the list can be walked without
//! opening a cursor at all, and a cursor opened afterwards opens on the row
//! that was picked out.
//!
//! Rebuilt every frame rather than on a turn boundary. A frame already
//! rewrites every cell of the map; a few dozen rows beside it is not the
//! cost worth being wrong about, and a panel that is a turn stale is the
//! kind of bug that survives to a release.

use atglyph_bevy::prelude::*;
use atglyph_render::Glyph;
use atglyph_rules::Relation;
use bevy::prelude::*;

use crate::facet::Facet;
use crate::focus::{Focus, InSight, Sighting, browse};
use crate::view::{Alert, Row};

/// Everything visible, in list order, and the row picked out.
///
/// Actors and things are separate lists because a panel almost always
/// wants them under separate headings, and because only one of them can
/// hurt you.
#[derive(Resource, Debug, Default)]
pub struct NearbyView {
    /// Actors, nearest first. Never includes the player.
    pub actors: Vec<Row>,
    /// Items and props on visible ground, nearest first: every item, and
    /// a prop when there is something to do with it or about it, as
    /// [`Business`] reads it.
    pub things: Vec<Row>,
    /// The row picked out, and where it stands: what [`Focus`] names, when
    /// that is in either list.
    pub focused: Option<Sighting>,
}

impl NearbyView {
    /// Every row in both lists, for an annotate system that does not care
    /// which is which.
    pub fn rows_mut(&mut self) -> impl Iterator<Item = &mut Row> {
        self.actors.iter_mut().chain(self.things.iter_mut())
    }

    /// The row for `entity`, in either list.
    pub fn row_mut(&mut self, entity: Entity) -> Option<&mut Row> {
        self.rows_mut().find(|r| r.entity == entity)
    }

    /// Whether `row` is the one picked out.
    pub fn is_focused(&self, row: &Row) -> bool {
        self.focused.is_some_and(|s| s.entity == row.entity)
    }

    /// Whether `entity` has a row in either list, as the game left them:
    /// an annotate system may take a row out, as one listed on a panel of
    /// its own is, and Tab steps only through what is still here.
    pub fn lists(&self, entity: Entity) -> bool {
        self.actors.iter().chain(&self.things).any(|r| r.entity == entity)
    }

    /// The row picked out, when it is still listed: [`focused`](Self::focused)
    /// less one the game took out of the lists, which the panel neither
    /// highlights nor marks on the map, since a mark with no row beside it
    /// reads as a cursor left behind.
    pub fn picked(&self) -> Option<Sighting> {
        self.focused.filter(|s| self.lists(s.entity))
    }

    /// How many actors are hostile to the player.
    pub fn threats(&self) -> usize {
        self.actors.iter().filter(|r| r.relation == Some(Relation::Hostile)).count()
    }

    /// Whether there is nothing to show.
    pub fn is_empty(&self) -> bool {
        self.actors.is_empty() && self.things.is_empty()
    }
}

/// Whether [`NearbyView`] lists the player's followers among the actors in
/// sight, which `collect_nearby` reads.
///
/// Listed by default: a follower is in sight like anyone else, and a game
/// with no party panel has nowhere else to show how it is doing. A game
/// whose party panel already lists them, which is everyone the
/// [`PartyView`](crate::view::PartyView) lists, leaves them out with
/// [`NearbyPanel::without_followers`](crate::NearbyPanel::without_followers),
/// or by inserting `NearbyFollowers::LeftOut` when it draws the view
/// itself, so a row is not spent twice on one follower.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NearbyFollowers {
    /// Listed with everyone else in sight.
    #[default]
    Listed,
    /// Left out: every living actor whose `Leader` is the player.
    LeftOut,
}

/// Whether [`NearbyView`]'s actor rows say what holds each actor, which
/// `note_statuses` reads, and in which of the game's words.
///
/// Off unless a game opts in, with
/// [`NearbyPanel::statuses`](crate::NearbyPanel::statuses) or by inserting
/// one when it draws the view itself: whether a rail spends its width on
/// what was done to a monster is a game's choice, and the words are the
/// game's.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct NearbyStatuses {
    /// The facet key each status is pushed under, which a panel may pick
    /// out by.
    pub key: &'static str,
    /// What a sleeper is said to be doing, in place of its tactic: a word
    /// the panel's activity words name, as a tactic's name is.
    pub asleep: &'static str,
}

/// Says beside each actor on the list what has been done to it: every
/// status on it with a [`badge`](atglyph_rules::StatusDef::badge), by its
/// registered name, as a facet under [`NearbyStatuses::key`], so what a
/// thrown potion or a zapped staff did is read off the screen and not only
/// off the log.
///
/// A status with no badge is what a monster is rather than what happened
/// to it, a regeneration it holds for life, and stays unsaid, as a vitals
/// panel leaves it off the player's own badges. A sleeper is said asleep
/// once, where the rail says what an actor is doing, as
/// [`NearbyStatuses::asleep`], and as one unaware of the player, since a
/// body asleep knows of nothing: not as a facet beside its name too.
///
/// In [`ViewSet::Annotate`](crate::ViewSet::Annotate), while a game has
/// inserted [`NearbyStatuses`]; nothing is said in a game with no
/// [`Registries`].
pub(crate) fn note_statuses(mut nearby: ResMut<NearbyView>, says: Res<NearbyStatuses>, registries: Option<Res<Registries>>, held: Query<&ActiveStatuses>) {
    let Some(registries) = registries.as_deref() else { return };
    for row in &mut nearby.actors {
        let Ok(held) = held.get(row.entity) else { continue };
        for active in held.iter() {
            let def = registries.statuses.get(active.id);
            if def.sleeps {
                (row.alert, row.doing) = (Some(Alert::Unaware), Some(says.asleep));
            } else if def.badge.is_some() {
                row.facets.push(Facet::new(says.key, registries.statuses.name(active.id)));
            }
        }
    }
}

/// Keeps [`NearbyView`] current, and walks its rows with the cursors'
/// "next" key while nothing is open.
///
/// Needs [`WorldMap`], to know which map the rows are on, and
/// [`CombatRules`], for the relation each row carries. Lists the player's
/// followers unless the game said otherwise with [`NearbyFollowers`], and
/// says what holds each actor when the game asked with
/// [`NearbyStatuses`].
pub struct NearbyViewPlugin;

impl Plugin for NearbyViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NearbyView>().init_resource::<Focus>().init_resource::<crate::CursorKeys>().init_resource::<NearbyFollowers>();
        app.needs::<CombatRules>("NearbyViewPlugin", "`CombatRules::new()`, for the relation each row carries")
            // Before either cursor reads the same keys, so the key that puts
            // a cursor away is not read again as letting go of what it left
            // picked out.
            .add_systems(Update, browse.in_set(EngineSet::Input).before(crate::view::inspect::move_cursor).before(crate::view::target::aim_cursor))
            .add_systems(Update, collect_nearby.in_set(crate::ViewSet::Collect))
            .add_systems(Update, note_statuses.run_if(resource_exists::<NearbyStatuses>).in_set(crate::ViewSet::Annotate));
    }

    fn finish(&self, app: &mut App) {
        atglyph_bevy::depends_on::<crate::UiPlugin>(app, "NearbyViewPlugin");
        // After the game's own, so its groups are listed first.
        crate::focus::declare_focus_controls(app);
    }
}

/// What a row is read off, once the list says it is in sight.
type Seen = (&'static Name, &'static Glyph, Option<&'static Health>, Option<&'static Faction>);

/// Who is looking, and at what.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct Around<'w, 's> {
    rules: Res<'w, CombatRules>,
    focus: Res<'w, Focus>,
    sight: InSight<'w, 's>,
    sighted: Res<'w, crate::focus::Sighted>,
    seen: Query<'w, 's, Seen>,
    factions: Query<'w, 's, &'static Faction>,
    watchers: Watchers<'w, 's>,
    noise: atglyph_bevy::NoiseRunning<'w>,
    heard: Query<'w, 's, &'static atglyph_bevy::Heard>,
    stacks: Query<'w, 's, &'static Stack>,
    work: crate::view::Workings<'w, 's>,
    doing: Query<'w, 's, &'static atglyph_bevy::Doing>,
    bodies: Query<'w, 's, (), With<atglyph_bevy::Remains>>,
    props: Business<'w, 's>,
    following: Following<'w, 's>,
}

/// Who follows whom, and whether the list leaves followers out.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct Following<'w, 's> {
    listing: Res<'w, NearbyFollowers>,
    /// The living, as the party view counts them: a body keeps its
    /// `Leader` and follows nobody.
    led: Query<'w, 's, &'static Leader, (With<Actor>, Without<Dead>)>,
}

impl Following<'_, '_> {
    /// Whether `entity` is left off the list as one of `me`'s followers.
    fn left_out(&self, me: Entity, entity: Entity) -> bool {
        *self.listing == NearbyFollowers::LeftOut && self.led.get(entity).is_ok_and(|l| l.0 == me)
    }
}

/// Whether a prop in sight is worth a row: something to do with it or
/// about it, by [`has_business`](atglyph_bevy::props::has_business) on its
/// definition or an [`Offers`] the game hung on it.
///
/// Read here rather than in `InSight`, because a barrel that can only be
/// broken is still something a cursor steps onto to aim at; it is only
/// beside what can be acted on that it is one row too many. A thing that
/// is no prop of a known kind, an item or a body, is always listed.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Business<'w, 's> {
    registries: Option<Res<'w, Registries>>,
    kinds: Query<'w, 's, (&'static PropKind, Option<&'static Offers>)>,
}

impl Business<'_, '_> {
    /// Whether `entity`, a thing in sight, has a row on the rail.
    pub fn listed(&self, entity: Entity) -> bool {
        let (Some(registries), Ok((kind, offers))) = (self.registries.as_deref(), self.kinds.get(entity)) else { return true };
        offers.is_some_and(|o| !o.0.is_empty()) || atglyph_bevy::props::has_business(registries.props.get(kind.0))
    }
}

/// Fills [`NearbyView`] from the player's viewshed, in [`InSight`]'s order,
/// with the player's followers or without them, as [`NearbyFollowers`]
/// says.
pub(crate) fn collect_nearby(mut view: ResMut<NearbyView>, around: Around) {
    view.actors.clear();
    view.things.clear();
    view.focused = None;
    let Some((me, _)) = around.sight.viewer() else { return };
    let mine = around.factions.get(me).ok();
    let list = around.sighted.list();
    view.focused = around.focus.within(list).copied();
    for sighting in list.iter().copied() {
        let Ok((name, glyph, health, faction)) = around.seen.get(sighting.entity) else { continue };
        if !sighting.actor && !around.props.listed(sighting.entity) {
            continue;
        }
        if sighting.actor && around.following.left_out(me, sighting.entity) {
            continue;
        }
        let count = around.stacks.get(sighting.entity).map_or(1, |s| s.count);
        let mut row = Row::new(sighting.entity, atglyph_core::noun::listed(name.as_str(), count), *glyph).at(sighting.distance);
        row.health = health.map(|h| (h.current, h.max));
        // A body keeps the side it fought for, which is what a mind is
        // told of it, but on the rail it is nobody's: a fallen friend in
        // the friend's tone, or a slain foe in the foe's, reads as still
        // standing.
        row.relation = match (mine, faction) {
            (Some(mine), Some(theirs)) if !around.bodies.contains(sighting.entity) => Some(around.rules.factions.relation(mine.0, theirs.0)),
            _ => None,
        };
        // Hunting outranks searching: something that has seen you is not
        // still wondering about a noise. Neither reading at all is a game
        // with neither stealth nor noise, or a thing that does not notice.
        // Something that had you and lost you to the unseen searches too:
        // it is on its way to where it last knew of you.
        if sighting.actor {
            let watching = around.watchers.running() && around.watchers.is_watcher(sighting.entity);
            let hunts = watching && around.watchers.sees(sighting.entity, me);
            let searches = (around.noise.get() && around.heard.get(sighting.entity).is_ok_and(|h| h.is_alert()))
                || (watching && around.watchers.remembers(sighting.entity, me));
            let knows_how =
                (around.watchers.running() && around.watchers.is_watcher(sighting.entity)) || (around.noise.get() && around.heard.get(sighting.entity).is_ok());
            row.alert = match (hunts, searches, knows_how) {
                (true, _, _) => Some(Alert::Hunting),
                (false, true, _) => Some(Alert::Searching),
                (false, false, true) => Some(Alert::Unaware),
                _ => None,
            };
            row.work = around.work.row(sighting.entity);
            row.doing = around.doing.get(sighting.entity).ok().and_then(|d| d.0);
        }
        if sighting.actor { view.actors.push(row) } else { view.things.push(row) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cursor::CursorKeys;
    use crate::harness::Stage;

    /// A game's annotate system: notes the one entity it was told about.
    #[derive(Resource)]
    struct Noted(Entity);

    fn note(mut view: ResMut<NearbyView>, noted: Res<Noted>) {
        if let Some(row) = view.row_mut(noted.0) {
            row.facets.push(Facet::new("wielding", "a rusty blade"));
        }
    }

    /// A follower in sight is listed like anyone else unless the game
    /// leaves followers out, and then only the follower goes: a stranger
    /// stays, and so does one following somebody else.
    #[test]
    fn followers_are_listed_by_default_and_left_out_only_when_the_game_says() {
        let actors = |listing: NearbyFollowers| {
            let mut stage = Stage::new_with(NearbyViewPlugin, |app| {
                app.insert_resource(listing);
            });
            let (me, follower) = (stage.player, stage.actor("follower", 'f', 2, 0));
            let stranger = stage.actor("stranger", 's', 3, 0);
            let theirs = stage.actor("another's", 'a', 4, 0);
            stage.app.world_mut().entity_mut(follower).insert(Leader(me));
            stage.app.world_mut().entity_mut(theirs).insert(Leader(stranger));
            stage.tick();
            stage.app.world().resource::<NearbyView>().actors.iter().map(|r| r.label.clone()).collect::<Vec<_>>()
        };
        assert_eq!(actors(NearbyFollowers::default()), ["follower", "stranger", "another's"], "listed by default");
        assert_eq!(actors(NearbyFollowers::LeftOut), ["stranger", "another's"], "left out when the game says");
    }

    #[test]
    fn the_rows_are_what_is_in_sight_nearest_first_and_never_the_player() {
        let mut stage = Stage::new(NearbyViewPlugin);
        stage.actor("far one", 'f', 6, 0);
        stage.actor("near one", 'n', 2, 0);
        stage.thing("a coin", '$', 1, 0);
        stage.tick();

        let view = stage.app.world().resource::<NearbyView>();
        let names: Vec<&str> = view.actors.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(names, vec!["near one", "far one"], "nearest first");
        assert!(!names.contains(&"you"), "the player is not in its own list");
        assert_eq!(view.actors[0].distance, 2);
        assert_eq!(view.actors[0].health, Some((10, 10)));
        assert_eq!(view.actors[0].relation, Some(Relation::Hostile));
        assert_eq!(view.things.len(), 1, "the coin is a thing, not an actor");
        assert_eq!(view.things[0].label, "a coin");
        assert_eq!(view.things[0].health, None);
        assert_eq!(view.threats(), 2);
    }

    /// Scenery is drawn but not listed: a lathe beside a crate leaves the
    /// crate the one thing in the rail, so a room of machinery does not
    /// bury what can be acted on.
    #[test]
    fn scenery_is_left_out_of_the_rows_and_a_thing_beside_it_is_not() {
        let mut stage = Stage::new(NearbyViewPlugin);
        let lathe = stage.thing("a lathe", '0', 1, 0);
        stage.app.world_mut().entity_mut(lathe).insert(atglyph_bevy::Scenery);
        stage.thing("a crate", '&', 2, 0);
        stage.tick();

        let view = stage.app.world().resource::<NearbyView>();
        let names: Vec<&str> = view.things.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(names, vec!["a crate"]);
    }

    /// Puts a prop of the kind called `name` from `props` `dx` cells east
    /// of the player, as a game's `spawn_prop` would, with a glyph to be
    /// drawn by.
    fn prop(stage: &mut Stage, name: &str, dx: i32) -> Entity {
        let registries = stage.app.world().resource::<Registries>().clone();
        let at = stage.at.offset(dx, 0);
        let prop = atglyph_bevy::props::spawn_prop(&mut stage.app.world_mut().commands(), &registries, registries.props.expect(name), at, MapId::ZERO);
        stage.app.world_mut().flush();
        stage.app.world_mut().entity_mut(prop).insert(Glyph::new(name.chars().next().unwrap_or('?'), Color::WHITE));
        prop
    }

    /// A barrel that can only be broken and a sac that bursts when it is
    /// are not listed, beside a chest, a lever and a found trap, which
    /// are; and a game that hangs an offer on a bare prop gives it its
    /// row. The look cursor's list keeps all of them.
    #[test]
    fn a_prop_is_listed_only_when_there_is_something_to_do_with_it_or_about_it() {
        let text = r#"#![enable(implicit_some)] [
            (name: "chest", glyph: '=', color: (r: 1, g: 1, b: 1), container: (contents: []), offers: [(verb: "open", time: 100)]),
            (name: "lever", glyph: '/', color: (r: 1, g: 1, b: 1), offers: [(verb: "pull", time: 100)]),
            (name: "trap", glyph: '^', color: (r: 1, g: 1, b: 1), hidden: (spot: 10), triggers: [(on: "entered", effects: [])]),
            (name: "barrel", glyph: '0', color: (r: 1, g: 1, b: 1), health: 4),
            (name: "sac", glyph: 'o', color: (r: 1, g: 1, b: 1), health: 1, triggers: [(on: "destroyed", effects: [])]),
            (name: "well", glyph: 'o', color: (r: 1, g: 1, b: 1)),
        ]"#;
        let mut stage = Stage::new(NearbyViewPlugin);
        let names = stage.app.world().resource::<Registries>().names();
        stage.app.world_mut().resource_mut::<Registries>().props = atglyph_rules::prop::load(text, &names).expect("the props load");
        let [chest, lever, trap, barrel, sac, well] = ["chest", "lever", "trap", "barrel", "sac", "well"].map(|name| prop(&mut stage, name, 1));
        // The trap has been spotted, and the well offers what the game
        // gave it, as a game whose wells choose a thing first does.
        stage.app.world_mut().entity_mut(trap).remove::<Hidden>();
        stage.app.world_mut().entity_mut(well).remove::<Scenery>().insert(Offers(vec![atglyph_bevy::Offering::new(atglyph_bevy::Verbs::OPEN, 100)]));
        stage.tick();

        let view = stage.app.world().resource::<NearbyView>();
        let mut listed: Vec<Entity> = view.things.iter().map(|r| r.entity).collect();
        listed.sort();
        let mut want = vec![chest, lever, trap, well];
        want.sort();
        assert_eq!(listed, want, "{:?}", view.things.iter().map(|r| &r.label).collect::<Vec<_>>());
        let sighted: Vec<Entity> = stage.app.world().resource::<crate::focus::Sighted>().list().iter().map(|s| s.entity).collect();
        assert!(sighted.contains(&barrel) && sighted.contains(&sac), "still in sight for a cursor to step onto");
    }

    #[test]
    fn a_row_says_whether_it_has_noticed_the_player_only_when_stealth_is_running() {
        // Stealth is decided in the minds' pass, so it needs the minds.
        let mut stage = Stage::new((NearbyViewPlugin, atglyph_bevy::MindsPlugin, atglyph_bevy::StealthPlugin));
        let player = stage.player;
        stage.app.world_mut().entity_mut(player).insert(Stealth::default());
        let hunting = stage.actor("hunting", 'h', 2, 0);
        let idle = stage.actor("idle", 'i', 3, 0);
        let blunt = stage.actor("blunt", 'b', 4, 0);
        for watcher in [hunting, idle] {
            stage.app.world_mut().entity_mut(watcher).insert(Notice(atglyph_rules::NoticeStats { certain: 1, chance_pct: 0, lit_bonus: 0, memory: 6 }));
        }
        let at = stage.at;
        stage.app.world_mut().get_mut::<Aware>(hunting).unwrap().0.insert(player, atglyph_rules::Awareness::Alert { at, stale_turns: 0 });
        stage.tick();

        let view = stage.app.world().resource::<NearbyView>();
        let alert = |e: Entity| view.actors.iter().find(|r| r.entity == e).map(|r| r.alert);
        assert_eq!(alert(hunting), Some(Some(Alert::Hunting)));
        assert_eq!(alert(idle), Some(Some(Alert::Unaware)));
        assert_eq!(alert(blunt), Some(None), "something with no Notice sees on sight and says nothing");

        let mut plain = Stage::new(NearbyViewPlugin);
        plain.actor("anyone", 'a', 2, 0);
        plain.tick();
        assert_eq!(plain.app.world().resource::<NearbyView>().actors[0].alert, None, "no stealth, no reading");
    }

    /// Something that had the player and lost it to the unseen is going to
    /// where it last knew of it: searching, not hunting, since nothing sees
    /// the unseen, and not idle, since it has not forgotten.
    #[test]
    fn a_row_that_lost_an_unseen_player_reads_searching_until_it_forgets() {
        let mut stage = Stage::new((NearbyViewPlugin, atglyph_bevy::MindsPlugin, atglyph_bevy::StealthPlugin));
        let player = stage.player;
        stage.app.world_mut().entity_mut(player).insert((Stealth::default(), atglyph_bevy::Unseen));
        let lost = stage.actor("lost", 'l', 2, 0);
        stage.app.world_mut().entity_mut(lost).insert(Notice(atglyph_rules::NoticeStats { certain: 1, chance_pct: 0, lit_bonus: 0, memory: 6 }));
        let at = stage.at;
        stage.app.world_mut().get_mut::<Aware>(lost).unwrap().0.insert(player, atglyph_rules::Awareness::Alert { at, stale_turns: 0 });
        stage.tick();

        let alert = |stage: &Stage| stage.app.world().resource::<NearbyView>().actors[0].alert;
        assert_eq!(alert(&stage), Some(Alert::Searching), "it has not forgotten, and it cannot see");
        stage.app.world_mut().get_mut::<Aware>(lost).unwrap().0.insert(player, atglyph_rules::Awareness::Unaware);
        stage.tick();
        assert_eq!(alert(&stage), Some(Alert::Unaware), "and once it has forgotten, it knows of nothing");
    }

    #[test]
    fn a_row_says_whether_it_is_going_to_look_at_a_sound_only_when_noise_is_running() {
        let rules = atglyph_bevy::NoiseRules { step: 0, strike: 0, door: 0, landing: 0, door_muffle: 0 };
        let mut stage = Stage::new((NearbyViewPlugin, atglyph_bevy::NoisePlugin::new(rules)));
        let listening = stage.actor("listening", 'l', 2, 0);
        let quiet = stage.actor("quiet", 'q', 3, 0);
        let deaf = stage.actor("deaf", 'd', 4, 0);
        let ear = atglyph_bevy::Hearing(atglyph_rules::HearingStats { threshold: 0, memory: 6 });
        for listener in [listening, quiet] {
            stage.app.world_mut().entity_mut(listener).insert(ear);
        }
        let at = stage.at;
        stage.app.world_mut().get_mut::<atglyph_bevy::Heard>(listening).unwrap().0 = atglyph_rules::Awareness::Alert { at, stale_turns: 0 };
        stage.tick();

        let view = stage.app.world().resource::<NearbyView>();
        let alert = |e: Entity| view.actors.iter().find(|r| r.entity == e).map(|r| r.alert);
        assert_eq!(alert(listening), Some(Some(Alert::Searching)), "it heard something and is on its way");
        assert_eq!(alert(quiet), Some(Some(Alert::Unaware)), "it heard nothing");
        assert_eq!(alert(deaf), Some(None), "something with no Hearing hears nothing and says nothing");

        let mut plain = Stage::new(NearbyViewPlugin);
        let anyone = plain.actor("anyone", 'a', 2, 0);
        plain.app.world_mut().entity_mut(anyone).insert(ear);
        plain.tick();
        assert_eq!(plain.app.world().resource::<NearbyView>().actors[0].alert, None, "no noise, no reading, Hearing or not");
    }

    #[test]
    fn something_out_of_sight_is_not_a_row() {
        let mut stage = Stage::new(NearbyViewPlugin);
        stage.actor("in sight", 'i', 3, 0);
        stage.actor("over the hill", 'o', 40, 40);
        stage.tick();

        let view = stage.app.world().resource::<NearbyView>();
        let names: Vec<&str> = view.actors.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(names, vec!["in sight"]);
    }

    #[test]
    fn a_game_facet_lands_on_the_row_it_names_and_on_no_other() {
        let mut stage = Stage::new(NearbyViewPlugin);
        let armed = stage.actor("armed one", 'a', 2, 0);
        stage.actor("bare one", 'b', 3, 0);
        stage.app.insert_resource(Noted(armed));
        stage.app.add_systems(Update, note.in_set(crate::ViewSet::Annotate));
        stage.tick();

        let view = stage.app.world().resource::<NearbyView>();
        let armed_row = view.actors.iter().find(|r| r.entity == armed).expect("the armed one is in sight");
        assert_eq!(armed_row.facets.len(), 1);
        assert_eq!(armed_row.facets[0].text, "a rusty blade");
        let bare = view.actors.iter().find(|r| r.label == "bare one").expect("in sight");
        assert!(bare.facets.is_empty(), "a facet does not spill onto its neighbour");
    }

    #[test]
    fn the_rows_are_rebuilt_each_frame_rather_than_appended_to() {
        let mut stage = Stage::new(NearbyViewPlugin);
        stage.actor("one", 'o', 2, 0);
        stage.tick();
        stage.tick();
        stage.tick();
        assert_eq!(stage.app.world().resource::<NearbyView>().actors.len(), 1, "three frames, one row");
    }

    #[test]
    fn tab_with_nothing_open_walks_the_rows_in_the_order_they_are_printed_and_escape_lets_go() {
        let mut stage = Stage::new(NearbyViewPlugin);
        let far = stage.actor("far one", 'f', 6, 0);
        let coin = stage.thing("a coin", '$', 1, 0);
        let near = stage.actor("near one", 'n', 2, 0);
        stage.tick();
        let focused = |stage: &Stage| stage.app.world().resource::<NearbyView>().focused.map(|s| s.entity);
        assert_eq!(focused(&stage), None, "nothing is picked out until the key");

        let keys = CursorKeys::default();
        for (want, why) in [(near, "the top row"), (far, "the next actor"), (coin, "then the things"), (near, "and round again")] {
            stage.press(keys.next);
            assert_eq!(focused(&stage), Some(want), "{why}");
        }
        let at = stage.app.world().resource::<NearbyView>().focused.map(|s| s.at);
        assert_eq!(at, Some(stage.at.offset(2, 0)), "and the view says where it stands, for the map");
        stage.press(keys.close);
        assert_eq!(focused(&stage), None, "let go");
    }

    /// A game's annotate system that lists the one it was told about on a
    /// panel of its own, so takes its row off the rail.
    fn list_elsewhere(mut view: ResMut<NearbyView>, noted: Res<Noted>) {
        view.actors.retain(|r| r.entity != noted.0);
    }

    #[test]
    fn tab_steps_only_through_the_rows_a_game_left_on_the_rail_and_a_cursor_left_on_one_taken_off_marks_nothing() {
        let mut stage = Stage::new(NearbyViewPlugin);
        let near = stage.actor("near one", 'n', 1, 0);
        let follower = stage.actor("follower", 'f', 2, 0);
        let far = stage.actor("far one", 'r', 3, 0);
        stage.app.insert_resource(Noted(follower));
        stage.app.add_systems(Update, list_elsewhere.in_set(crate::ViewSet::Annotate));
        stage.tick();
        let focused = |stage: &Stage| stage.app.world().resource::<NearbyView>().focused.map(|s| s.entity);
        let keys = CursorKeys::default();
        for (want, why) in [(near, "the top row"), (far, "past the one listed elsewhere"), (near, "and round again")] {
            stage.press(keys.next);
            assert_eq!(focused(&stage), Some(want), "{why}");
        }

        // Picked out some other way, as by the look cursor: still the
        // focus, but nothing on the rail to highlight or mark.
        stage.app.world_mut().resource_mut::<Focus>().set(Some(follower));
        stage.tick();
        let view = stage.app.world().resource::<NearbyView>();
        assert_eq!(view.focused.map(|s| s.entity), Some(follower), "the focus is the cursor's to keep");
        assert_eq!(view.picked(), None, "but the rail picks out nothing it does not list");
    }

    #[test]
    fn a_row_that_leaves_sight_is_no_longer_picked_out() {
        let mut stage = Stage::new(NearbyViewPlugin);
        let one = stage.actor("one", 'o', 2, 0);
        stage.tick();
        stage.press(CursorKeys::default().next);
        assert_eq!(stage.app.world().resource::<NearbyView>().focused.map(|s| s.entity), Some(one));
        let far = stage.at.offset(40, 40);
        stage.app.world_mut().get_mut::<Position>(one).unwrap().0 = far;
        stage.tick();
        stage.tick();
        assert_eq!(stage.app.world().resource::<NearbyView>().focused, None, "out of sight, so nothing is highlighted");
    }

    #[test]
    fn a_body_on_the_rail_is_on_nobodys_side_whoever_it_fought_for() {
        let mut stage = Stage::new(NearbyViewPlugin);
        let body = stage.thing("goblin remains", '%', 2, 0);
        let foe = stage.actor("foe", 'f', 3, 0);
        let side = *stage.app.world().get::<Faction>(foe).expect("a side");
        stage.app.world_mut().entity_mut(body).insert((side, atglyph_bevy::Remains { since: 0, credit: None }));
        stage.tick();
        let view = stage.app.world().resource::<NearbyView>();
        assert_eq!(view.things.iter().find(|r| r.entity == body).map(|r| r.relation), Some(None), "no side: {:?}", view.things);
        assert_eq!(view.actors[0].relation, Some(Relation::Hostile), "while the foe beside it keeps its own");
    }

    #[test]
    fn a_row_says_which_tactic_decided_an_actors_last_turn() {
        let mut stage = Stage::new(NearbyViewPlugin);
        let runner = stage.actor("runner", 'r', 2, 0);
        let still = stage.actor("still", 's', 3, 0);
        stage.app.world_mut().entity_mut(runner).insert(atglyph_bevy::Doing(Some("flee_when_hurt")));
        stage.tick();
        let view = stage.app.world().resource::<NearbyView>();
        let doing = |e: Entity| view.actors.iter().find(|r| r.entity == e).unwrap().doing;
        assert_eq!(doing(runner), Some("flee_when_hurt"));
        assert_eq!(doing(still), None, "and nothing for an actor no tactic has decided for");
    }

    #[test]
    fn a_row_says_what_an_actor_is_working_on_and_how_long_it_has_left() {
        let mut stage = Stage::new_with(NearbyViewPlugin, |app| {
            atglyph_bevy::AddWork::add_work(app, "mending");
        });
        let kind = stage.app.world().resource::<atglyph_bevy::WorkKinds>().get("mending").unwrap();
        let mender = stage.actor("mender", 'm', 2, 0);
        let rag = stage.actor("rag doll", 'r', 3, 0);
        let mut work = atglyph_rules::Work::new(kind, 10).on(rag);
        work.done = 6;
        stage.app.world_mut().entity_mut(mender).insert(atglyph_bevy::Working(work));
        stage.tick();
        let view = stage.app.world().resource::<NearbyView>();
        let row = view.actors.iter().find(|r| r.entity == mender).unwrap();
        assert_eq!(row.work, Some(crate::view::WorkRow { doing: "mending".into(), target: Some("rag doll".into()), left: 4 }));
        assert_eq!(view.actors.iter().find(|r| r.entity == rag).unwrap().work, None, "and nothing on one not at work");
    }

    /// A stage whose statuses are a badged hold, an unbadged mend and a
    /// sleep, with each actor named holding what it says.
    fn held_stage(says: Option<NearbyStatuses>) -> (Stage, [Entity; 4]) {
        let mut stage = Stage::new_with(NearbyViewPlugin, |app| {
            if let Some(says) = says {
                app.insert_resource(says);
            }
        });
        let defs = atglyph_rules::Registry::from_defs(vec![
            atglyph_rules::StatusDef { badge: Some('P'), ..atglyph_rules::StatusDef::new("held fast").skips_turns() },
            atglyph_rules::StatusDef::new("knitting").mends(1),
            atglyph_rules::StatusDef::new("dozing").sleeps(),
        ])
        .unwrap();
        stage.app.world_mut().resource_mut::<Registries>().statuses = defs.clone();
        let holding = |stage: &mut Stage, name: &str, dx: i32, status: Option<&str>| {
            let e = stage.actor(name, name.chars().next().unwrap_or('?'), dx, 0);
            let (mut held, mut stats) = (atglyph_rules::Statuses::new(), atglyph_rules::Stats::new());
            if let Some(status) = status {
                held.apply(defs.expect(status), 5, None, &defs, &mut stats);
            }
            stage.app.world_mut().entity_mut(e).insert(ActiveStatuses(held));
            e
        };
        let held = holding(&mut stage, "held", 1, Some("held fast"));
        let mending = holding(&mut stage, "mending", 2, Some("knitting"));
        let sleeper = holding(&mut stage, "sleeper", 3, Some("dozing"));
        let free = holding(&mut stage, "free", 4, None);
        stage.tick();
        (stage, [held, mending, sleeper, free])
    }

    fn row(stage: &Stage, who: Entity) -> Row {
        stage.app.world().resource::<NearbyView>().actors.iter().find(|r| r.entity == who).cloned().expect("on the list")
    }

    fn said(stage: &Stage, who: Entity) -> Vec<String> {
        row(stage, who).facets.iter().map(|f| f.text.clone()).collect()
    }

    /// With the game's say-so, an actor in sight that a badged status
    /// holds reads that status by name, one holding only a status with no
    /// badge reads nothing beside its name, and nor does one holding none.
    #[test]
    fn an_actor_is_said_with_what_holds_it_by_name_and_nothing_else_once_the_game_asks() {
        let (stage, [held, mending, _, free]) = held_stage(Some(NearbyStatuses { key: "status", asleep: "asleep" }));
        assert_eq!(said(&stage, held), vec!["held fast".to_string()]);
        assert_eq!(said(&stage, mending), Vec::<String>::new(), "a status with no badge is what it is, not what was done to it");
        assert_eq!(said(&stage, free), Vec::<String>::new());
    }

    /// A sleeper is said asleep once, as what it is doing and unaware, with
    /// no facet saying so twice.
    #[test]
    fn a_sleeper_is_said_asleep_once_as_what_it_is_doing_and_unaware() {
        let (stage, [_, _, sleeper, free]) = held_stage(Some(NearbyStatuses { key: "status", asleep: "asleep" }));
        assert_eq!(said(&stage, sleeper), Vec::<String>::new(), "not beside its name too");
        assert_eq!((row(&stage, sleeper).doing, row(&stage, sleeper).alert), (Some("asleep"), Some(Alert::Unaware)));
        assert_eq!(row(&stage, free).doing, None, "awake, it is doing nothing of the kind");
    }

    /// Without the game's say-so nothing is said of what holds anyone.
    #[test]
    fn nothing_is_said_of_what_holds_an_actor_unless_the_game_asks() {
        let (stage, [held, _, sleeper, _]) = held_stage(None);
        assert_eq!(said(&stage, held), Vec::<String>::new());
        assert_eq!(row(&stage, sleeper).doing, None);
    }
}
