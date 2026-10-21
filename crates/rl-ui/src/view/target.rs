//! The targeting cursor: where an ability or a thing used at somewhere
//! would land, a thrown item fly, or a shot go, before it does.
//!
//! Two pieces that only look like one, the way the look cursor is. The
//! cursor is behaviour the engine owns: it opens onto the nearest thing
//! worth aiming at, steps with the direction keys, cycles through what the
//! aim can take, refuses to leave the loaded window, and on confirm writes
//! the [`Use`], the [`UseItemAt`], the [`Throw`] or the [`Attack`] itself.
//! The picture is a view: the cells it would cover from here and who it
//! would hit, resolved by [`Bystanders::land`] for an ability and for a
//! thing's aimed use alike, [`flight`] for a throw and [`shot`] for a shot,
//! the calls the resolvers act on, so what is shown is what will happen.
//!
//! A thing not yet known, [`Unknown`], is the exception, since its shape,
//! its reach, what it wants and whether it wants its cell seen are each
//! part of what it is, and a preview that showed any of them, or refused an
//! aim for one, would tell the player its kind in a few keys and for
//! nothing. Its preview is a plain line from its user to the cursor and the
//! cursor's cell: no footprint, no reach, nothing painted out of reach, no
//! reason it would be refused but its not being carried, and nobody named
//! as caught, since who it catches tells its shape. It cycles to anyone.
//! Confirmed, it is used and spent, and lands by its true targeting where
//! its true flight stops, which the items resolver works out.
//!
//! Cycling steps through what is in sight in the nearby list's order, held
//! to what the aim can take by [`Aim::cycles_to`]: foes for an ability or a
//! use aimed at foes, allies and the user for one aimed at allies, and
//! anything in sight, things included, for one aimed at the ground, which
//! would otherwise have nothing to stop on but cells. A throw or a shot
//! stops on any actor but whoever sends it. The direction keys still reach
//! every cell, and wherever the cursor goes the [`Focus`] follows.
//!
//! What an aim was confirmed at is remembered apart from the focus, as
//! `LastAimed`, and the next aim opens on it when nothing is picked out.
//! The two are kept apart because they answer different questions: the
//! focus is what the nearby list shows picked out, and an aim that has been
//! fired or put away leaves nothing picked out, so the list is not left
//! pointing at a droid long after the grenade has landed; what the player
//! last threw at is only where the next throw starts.
//!
//! The cursor is a modal, declared under the name `target`, so a game
//! gates its movement keys on [`no_modal`](crate::no_modal) and gets the
//! exclusion from the bag and the look cursor for free.
//!
//! A game opens it by writing [`AimAt`] for an ability, `AimUse` for a
//! thing used at somewhere, [`AimThrow`] for something carried, or
//! [`AimFire`] for a shot, which is all the code the key needs: the engine
//! picks the first target, runs the cursor, and spends the turn. Nothing
//! about which key does it is the engine's business.

use crate::controls::AddControls;
use crate::modal::AddModal;
use bevy::prelude::*;
use rl_bevy::prelude::*;
use rl_bevy::{Aimed, Bystanders, Landed, Offered, Source, flight, shot};
use rl_core::Point;
use rl_grid::TargetMode;
use rl_render::Glyph;
use rl_rules::ability::{AbilityId, Aim, Blocked, Targeting};

use crate::cursor::{CursorInput, CursorKeys, Steer};
use crate::focus::{Focus, Sighting};
use crate::modal::{ModalId, Modals};
use crate::view::Row;

/// The name the targeting cursor's modal is declared under.
pub(crate) const TARGET_MODAL: &str = "target";

/// Open the targeting cursor on an ability.
///
/// What a game writes when an ability key is pressed. The engine answers
/// it: opening the cursor, aiming it, and writing the [`Use`] when the
/// player confirms. An ability whose [`Aim`] needs no cursor is used at
/// once and no screen opens, so a game binds every ability the same way
/// and never asks which kind it is.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AimAt {
    /// Who is aiming.
    pub user: Entity,
    /// What they are aiming.
    pub ability: AbilityId,
}

/// Open the targeting cursor on something carried whose use is aimed, to
/// use it at somewhere: a bolt sent from the pack at a foe.
///
/// What the bag writes when its use key is pressed on such a thing, and
/// what a game's own use key may write for any thing at all. The cursor
/// opens as it does for an ability aimed the same way, previews through
/// [`Bystanders::land`] the footprint and who stands in it, and writes the
/// [`UseItemAt`] when the player confirms. A thing whose use lands on its
/// user is used at once and no screen opens, and one whose use asks for
/// another of its user's things opens the bag choosing for it, when the
/// game has the bag, so a game binds every use the same way and never asks
/// which kind it is; one that cannot be used now, by
/// [`may_use`](rl_bevy::may_use), an empty one its user knows or one worn to
/// be used and not worn, opens nothing. Put away, it spends nothing and
/// passes no turn.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AimUse {
    /// Who is using it.
    pub user: Entity,
    /// What they are using.
    pub item: Entity,
}

/// Open the targeting cursor on something carried, to throw it.
///
/// What a game writes when its throw key is pressed. The cursor opens on the
/// nearest foe, previews the flight, and writes the [`Throw`] when the
/// player confirms. Needs [`ThrowingPlugin`] to be resolved, and does
/// nothing for an item that cannot be thrown.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AimThrow {
    /// Who is throwing.
    pub user: Entity,
    /// What they are throwing.
    pub item: Entity,
}

/// Open the targeting cursor on a shot with the user's [`RangedAttack`],
/// its own or the one a worn item lends it, as [`Loadout`] reads them.
///
/// What a game writes when its fire key is pressed. The cursor opens on the
/// nearest foe, previews through [`shot`] the line the shot takes and
/// whether it reaches, and writes an [`Attack`] on whoever stands under the
/// cursor when the player confirms. Needs [`CombatPlugin`] to be resolved,
/// and does nothing for a user with no [`RangedAttack`] of either kind.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AimFire {
    /// Who is firing.
    pub user: Entity,
}

/// Distances for the targeting box: to the cursor, the edge of no range
/// penalty, and the furthest reach.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AimRange {
    /// Chebyshev distance from the user to the cursor.
    pub distance: i32,
    /// The furthest cell with no range penalty.
    pub effective: i32,
    /// The furthest cell reached.
    pub max: i32,
}

/// What is being aimed, and where it would land.
#[derive(Resource, Debug, Default)]
pub struct TargetView {
    /// The ability being aimed, while the cursor is open on one.
    pub ability: Option<AbilityId>,
    /// The thing whose use is being aimed, while the cursor is open on one.
    pub using: Option<Entity>,
    /// The item being thrown, while the cursor is open on a throw.
    pub throwing: Option<Entity>,
    /// Whether the cursor is open on a shot.
    pub firing: bool,
    /// What is being aimed, by the name the game gave it: the ability's, or
    /// the used or thrown item's. Empty for a shot, which has no name of its
    /// own.
    pub what: String,
    /// Whether what is named is a thing not yet known, [`Unknown`]: the box
    /// says so, and `span` is left out, since how far a thing reaches is
    /// part of what it is. A thing used at somewhere withholds the rest of
    /// what its targeting would tell as well: `path` is a plain line to the
    /// cursor, `cells` the cursor's cell alone, and `beyond`, `why` and
    /// `targets` are empty, with `legal` only while it is carried. A thrown
    /// one is previewed as it flies and where it comes down, which is all
    /// a throw of anything paints, but gives no `odds`, since whether it is
    /// rolled at all, only a thing that strikes being rolled, is its own.
    pub unknown: bool,
    /// Who is aiming it.
    pub user: Option<Entity>,
    /// The world tile the cursor is on.
    pub cursor: Point,
    /// Every cell the ability would cover from here.
    pub cells: Vec<Point>,
    /// The cells a projectile would fly through, landing included.
    pub path: Vec<Point>,
    /// Where a projectile would stop.
    pub landing: Option<Point>,
    /// The cells from where the aim stops short to where it was pointed,
    /// the stop excluded and the cursor included, when it stops short:
    /// what the overlay paints in the tone for bad news, so a target out
    /// of reach reads as out of reach. Empty when the aim reaches.
    pub beyond: Vec<Point>,
    /// Whether the resolver would accept this aim.
    pub legal: bool,
    /// Why it would not, empty when it would.
    pub why: Vec<Blocked>,
    /// Everyone under the footprint the ability's aim wants there.
    pub targets: Vec<Row>,
    /// How far the cursor is from the user, how far the weapon reaches
    /// with no penalty, and how far it reaches at all. `None` with no aim.
    pub span: Option<AimRange>,
    /// The chance of landing on whoever the aim would strike, and what
    /// shaped it, from the same call the resolver rolls by, at the accuracy
    /// the user's [`Seems`](crate::view::Seems) takes off. `None` when
    /// nobody would be struck, the game's model does not roll, or the aim
    /// is an ability, which is never rolled.
    pub odds: Option<rl_rules::Odds>,
    /// Whether `odds` is worked out at the accuracy the user seems to have
    /// rather than the one it has, for a game that says why in its own
    /// words.
    pub seeming: bool,
    /// What the game added about the aim, in
    /// [`ViewSet::Annotate`](crate::ViewSet), which the box prints under
    /// the chance to hit: a game whose chance is `seeming` says why here,
    /// as it does under the look cursor's forecast.
    pub facets: Vec<crate::facet::Facet>,
}

impl TargetView {
    /// Whether the cursor is up.
    pub fn aiming(&self) -> bool {
        self.ability.is_some() || self.using.is_some() || self.throwing.is_some() || self.firing
    }
}

/// Adds the targeting cursor and keeps [`TargetView`] current.
///
/// Needs [`WorldMap`]; an ability needs the abilities the game loaded, a
/// use needs [`ItemsPlugin`] to be resolved and the effects plugin to land,
/// a throw needs [`ThrowingPlugin`] to be resolved, and a shot
/// [`CombatPlugin`]. Its keys are [`CursorKeys`], the ones the look cursor
/// answers to.
pub struct TargetViewPlugin;

impl Plugin for TargetViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TargetView>()
            .init_resource::<CursorKeys>()
            .init_resource::<Focus>()
            .init_resource::<LastAimed>()
            .add_message::<AimAt>()
            .add_message::<AimUse>()
            .add_message::<AimThrow>()
            .add_message::<AimFire>();
        // The intents the cursor writes on confirm, registered whether or
        // not the game added the plugin that resolves each: a game that
        // throws and has no abilities still runs this cursor.
        app.add_message::<Intent<Use>>()
            .add_message::<Intent<UseItem>>()
            .add_message::<Intent<UseItemAt>>()
            .add_message::<Intent<Throw>>()
            .add_message::<Intent<Attack>>();
        // `Modals` is plain data, so this plugin makes sure it exists rather
        // than panicking when added before `UiPlugin`.
        app.add_modal(TARGET_MODAL);
        app.add_systems(Update, aim_cursor.in_set(EngineSet::Input)).add_systems(Update, collect_target.in_set(crate::ViewSet::Collect));
    }

    fn finish(&self, app: &mut App) {
        rl_bevy::depends_on::<crate::UiPlugin>(app, "TargetViewPlugin");
        // After the game's own, so its groups are listed first.
        crate::focus::declare_cursor_controls(app);
        app.add_control(crate::focus::CURSOR_GROUP, "confirm an aim", crate::controls::EngineKey::Confirm);
    }
}

/// The id of the targeting modal, for a game gating its own systems.
///
/// # Panics
/// Panics if [`TargetViewPlugin`] was not added.
pub fn target_modal(modals: &Modals) -> ModalId {
    modals.get(TARGET_MODAL).expect("TargetViewPlugin declares the target modal")
}

/// What the cursor is aiming, when it is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pointing {
    Ability(AbilityId),
    Use(Entity),
    Throw(Entity),
    Fire,
}

impl Pointing {
    /// What the view says is being aimed, if anything is.
    fn of(view: &TargetView) -> Option<Pointing> {
        match (view.ability, view.using, view.throwing, view.firing) {
            (Some(ability), _, _, _) => Some(Pointing::Ability(ability)),
            (None, Some(item), _, _) => Some(Pointing::Use(item)),
            (None, None, Some(item), _) => Some(Pointing::Throw(item)),
            (None, None, None, true) => Some(Pointing::Fire),
            (None, None, None, false) => None,
        }
    }

    /// Writes this into the view, over whatever was aimed before.
    fn into_view(self, view: &mut TargetView, user: Entity) {
        view.ability = match self {
            Pointing::Ability(ability) => Some(ability),
            _ => None,
        };
        view.using = match self {
            Pointing::Use(item) => Some(item),
            _ => None,
        };
        view.throwing = match self {
            Pointing::Throw(item) => Some(item),
            _ => None,
        };
        view.firing = self == Pointing::Fire;
        view.user = Some(user);
    }
}

/// What the last aim was confirmed at, where the next one opens when
/// nothing is picked out in the nearby list.
///
/// It may name something that has since left sight or died, or that the
/// next aim cannot take: it is read against the aim's own candidates, and
/// a stale one reads as none. A cancelled aim leaves it as it was, since
/// only a thing actually thrown or shot at is worth coming back to.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LastAimed(pub Option<Entity>);

/// Everything the cursor steers by.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Aiming<'w, 's> {
    input: CursorInput<'w>,
    map: Res<'w, WorldMap>,
    abilities: Option<Res<'w, Abilities>>,
    occupancy: Res<'w, Occupancy>,
    focus: ResMut<'w, Focus>,
    last: ResMut<'w, LastAimed>,
    sighted: Res<'w, crate::focus::Sighted>,
    bystanders: Bystanders<'w, 's>,
    users: Query<'w, 's, (&'static Position, Option<&'static Viewshed>)>,
    living: Query<'w, 's, &'static Health, Without<Dead>>,
    missiles: Query<'w, 's, (), With<Throwable>>,
    /// Whether a user has a shot at all: its own, or a worn gun's, read
    /// the way the resolver fires it, so a gun in hand opens the cursor.
    loadout: Loadout<'w, 's>,
    /// What a thing does, for how its use is pointed.
    triggers: Query<'w, 's, &'static Triggers>,
    /// What a user carries and wears, and what a thing used out of it
    /// spends and is worn in, for whether it may be used at all.
    bags: Query<'w, 's, (&'static Inventory, Option<&'static Equipped>)>,
    things: Query<'w, 's, (Option<&'static Consumable>, Has<Wearable>, Has<Unknown>)>,
}

impl Aiming<'_, '_> {
    /// How `item`'s use is pointed, when it is pointed somewhere.
    fn use_targeting(&self, item: Entity) -> Option<Targeting> {
        self.triggers.get(item).ok().and_then(Triggers::targeting)
    }

    /// Whether `user` may use `item` now, wherever it is pointed, by the
    /// rule the items resolver refuses a use by.
    fn may_use(&self, user: Entity, item: Entity) -> bool {
        let (Ok((bag, worn)), Ok((consumable, wearable, unknown))) = (self.bags.get(user), self.things.get(item)) else { return false };
        rl_bevy::may_use(item, bag, worn, consumable, wearable, unknown)
    }

    /// The aim a pointing is judged by. A throw and a shot are for foes,
    /// which is where they open. A thing not yet known is aimed at anyone,
    /// since cycling only to what its own aim wants would tell it.
    fn aim(&self, pointing: Pointing) -> Aim {
        match pointing {
            Pointing::Ability(ability) => self.abilities.as_deref().map_or(Aim::Foe, |a| a.get(ability).aim),
            Pointing::Use(item) if self.things.get(item).is_ok_and(|(_, _, unknown)| unknown) => Aim::Anyone,
            Pointing::Use(item) => self.use_targeting(item).map_or(Aim::Foe, |t| t.aim),
            Pointing::Throw(_) | Pointing::Fire => Aim::Foe,
        }
    }

    /// What the cursor stops on when cycling, in the nearby list's order,
    /// with the user first when the aim can take its user.
    ///
    /// Held to what the user can see as well as the player, for a user
    /// that is not the player.
    fn candidates(&self, user: Entity, pointing: Pointing) -> Vec<Sighting> {
        let Ok((from, sight)) = self.users.get(user) else { return Vec::new() };
        let aim = self.aim(pointing);
        let me = Sighting { entity: user, at: from.0, actor: true, distance: 0 };
        std::iter::once(me)
            .chain(self.sighted.list().iter().copied().filter(|s| s.entity != user))
            .filter(|s| {
                let is_user = s.entity == user;
                let seen = is_user || sight.is_none_or(|v| v.can_see(s.at));
                let relation = self.bystanders.relation(user, s.entity);
                seen && match pointing {
                    Pointing::Ability(_) | Pointing::Use(_) => aim.cycles_to(relation, is_user, s.actor),
                    // Something thrown or shot can be sent at anyone but
                    // whoever sends it.
                    Pointing::Throw(_) | Pointing::Fire => s.actor && !is_user,
                }
            })
            .collect()
    }

    /// Where the cursor opens: on what is picked out, when the aim can take
    /// it, then on what the last aim was fired at, and otherwise on the
    /// nearest thing worth aiming at.
    ///
    /// [`Aim::worth_aiming_at`] is the rule the mind's tactic aims by, so
    /// the cursor opens on what a monster would have picked, the user's own
    /// cell included when there is a hurt user to mend, and a player who
    /// just presses confirm gets the sensible shot. `None` when there is
    /// neither, and the cursor opens on the user.
    fn opening(&self, user: Entity, pointing: Pointing) -> Option<Sighting> {
        let list = self.candidates(user, pointing);
        if let Some(picked) = self.focus.within(&list) {
            return Some(*picked);
        }
        if let Some(last) = self.last.0.and_then(|entity| list.iter().find(|s| s.entity == entity)) {
            return Some(*last);
        }
        let aim = self.aim(pointing);
        list.into_iter().find(|s| {
            let Ok(health) = self.living.get(s.entity) else { return false };
            aim.worth_aiming_at(self.bystanders.relation(user, s.entity), s.entity == user, health.current < health.max)
        })
    }
}

/// What the cursor may be asked to open on this frame.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct AimRequests<'w, 's> {
    abilities: MessageReader<'w, 's, AimAt>,
    uses: MessageReader<'w, 's, AimUse>,
    throws: MessageReader<'w, 's, AimThrow>,
    fires: MessageReader<'w, 's, AimFire>,
}

/// What the cursor writes when the player confirms, or at once for a use
/// that needs no aiming.
#[derive(bevy::ecs::system::SystemParam)]
pub struct AimIntents<'w> {
    uses: MessageWriter<'w, Intent<Use>>,
    item_uses: MessageWriter<'w, Intent<UseItem>>,
    aimed_uses: MessageWriter<'w, Intent<UseItemAt>>,
    throws: MessageWriter<'w, Intent<Throw>>,
    attacks: MessageWriter<'w, Intent<Attack>>,
}

/// Opens, steps, cycles, confirms and cancels the cursor.
pub(crate) fn aim_cursor(mut view: ResMut<TargetView>, mut modals: ResMut<Modals>, mut requests: AimRequests, mut intents: AimIntents, mut aiming: Aiming) {
    let modal = target_modal(&modals);

    // The last request of a frame is the one aimed, and the cursor opens
    // once for it.
    let mut asked: Option<(Entity, Pointing)> = None;
    for request in requests.abilities.read() {
        let (Some(abilities), Ok((from, _))) = (aiming.abilities.as_deref(), aiming.users.get(request.user)) else { continue };
        // An ability that wants no cursor is used where it stands. A game
        // binds every ability the same way and never asks which kind it is.
        if !abilities.get(request.ability).aim.needs_cursor() {
            intents.uses.write(Intent::new(request.user, Use { ability: request.ability, aim: from.0 }));
            continue;
        }
        asked = Some((request.user, Pointing::Ability(request.ability)));
    }
    for request in requests.uses.read() {
        // A thing whose use asks for another of its user's things is chosen
        // for in the bag, which reads the same request, and not aimed.
        if aiming.triggers.get(request.item).is_ok_and(|t| t.asks().is_some()) {
            continue;
        }
        // A thing whose use lands on its user is used where it stands, as
        // an ability that wants no cursor is, and refused there when it
        // cannot be.
        if aiming.use_targeting(request.item).is_none() {
            intents.item_uses.write(Intent::new(request.user, UseItem(request.item)));
            continue;
        }
        // One that cannot be used now, wherever it is pointed, opens no
        // aim, as a thing that cannot be thrown opens none for a throw: a
        // cursor that showed it as legal would be refused on confirm.
        if aiming.may_use(request.user, request.item) {
            asked = Some((request.user, Pointing::Use(request.item)));
        }
    }
    for request in requests.throws.read() {
        if aiming.missiles.contains(request.item) {
            asked = Some((request.user, Pointing::Throw(request.item)));
        }
    }
    for request in requests.fires.read() {
        if aiming.loadout.ranged(request.user).is_some() {
            asked = Some((request.user, Pointing::Fire));
        }
    }
    if let Some((user, pointing)) = asked
        && let Ok((from, _)) = aiming.users.get(user)
    {
        let from = from.0;
        let opens_on = aiming.opening(user, pointing);
        pointing.into_view(&mut view, user);
        view.cursor = opens_on.map_or(from, |s| s.at);
        if let Some(target) = opens_on {
            aiming.focus.set(Some(target.entity));
        }
        if !modals.is_open(modal) {
            modals.open(modal);
        }
        // Opened this frame, and not steered this frame: the key that asked
        // for the cursor is still down, and if it was a menu's confirm key
        // it would confirm the aim before the player had seen it.
        return;
    }

    if !modals.is_top(modal) {
        return;
    }
    let (Some(pointing), Some(user)) = (Pointing::of(&view), view.user) else {
        // The cursor is up with nothing in it, which can only mean the
        // aimer is gone. Put it away rather than leave input trapped.
        modals.close_one(modal);
        return;
    };
    if aiming.users.get(user).is_err() {
        // The aimer left the world: there is nothing to aim from.
        close(&mut view, &mut modals, modal);
        return;
    }
    // Steered on a copy, so the candidates can be borrowed while the focus
    // moves, and written back only when it did.
    let mut focus = *aiming.focus;
    let steer = aiming.input.steer(&mut view.cursor, &mut focus, aiming.map.window_tiles(), || aiming.candidates(user, pointing));
    if focus != *aiming.focus {
        *aiming.focus = focus;
    }
    match steer {
        Steer::Close => {
            aiming.focus.clear();
            close(&mut view, &mut modals, modal);
        }
        Steer::Confirm => {
            // Refused aims are the resolver's to report, not the cursor's: a
            // player who insists gets the refusal in the log with its reason,
            // which is better than a key that does nothing.
            match pointing {
                Pointing::Ability(ability) => {
                    intents.uses.write(Intent::new(user, Use { ability, aim: view.cursor }));
                }
                Pointing::Use(item) => {
                    intents.aimed_uses.write(Intent::new(user, UseItemAt { item, at: view.cursor }));
                }
                Pointing::Throw(item) => {
                    intents.throws.write(Intent::new(user, Throw { item, at: view.cursor }));
                }
                Pointing::Fire => {
                    // A shot is at someone rather than at a cell. With nobody
                    // under the cursor there is no attack to write, and the
                    // box already says there is no target, so the cursor
                    // stays up for the player to move.
                    let Some(target) = mark(&aiming.occupancy, user, view.cursor, |who| aiming.living.contains(who)) else { return };
                    intents.attacks.write(Intent::new(user, Attack(target)));
                }
            }
            // What it was fired at is remembered for the next aim, and is
            // no longer picked out in the list.
            aiming.last.0 = aiming.focus.get();
            aiming.focus.clear();
            close(&mut view, &mut modals, modal);
        }
        Steer::Moved | Steer::Stay => {}
    }
}

/// Who a shot at `cell` is at: whoever living stands there, never the one
/// shooting. The one rule the cursor writes the attack by and the preview
/// names its target by.
fn mark(occupancy: &Occupancy, user: Entity, cell: Point, alive: impl Fn(Entity) -> bool) -> Option<Entity> {
    occupancy.first_at(cell).filter(|who| *who != user && alive(*who))
}

/// Puts the cursor away and forgets what it was aiming.
fn close(view: &mut TargetView, modals: &mut Modals, modal: ModalId) {
    forget(view);
    modals.close_one(modal);
}

/// Forgets what the cursor was aiming.
fn forget(view: &mut TargetView) {
    view.ability = None;
    view.using = None;
    view.throwing = None;
    view.firing = false;
    view.what.clear();
    view.unknown = false;
    view.user = None;
    view.cells.clear();
    view.path.clear();
    view.beyond.clear();
    view.targets.clear();
    view.why.clear();
    view.span = None;
    view.odds = None;
    view.seeming = false;
    view.facets.clear();
}

/// Anything the cursor might land on, as a row is built from it.
///
/// The name and the glyph are optional here and required by the other
/// views, and the difference matters: a nameless or glyphless row is one a
/// list can leave out, but such a target is one the ability will hit
/// anyway. Dropping it would make the box say "nothing" over a monster
/// about to be burned, so `targets` counts what the footprint catches and
/// the cosmetics are filled in where they exist.
type Standing = (Entity, &'static Position, Option<&'static Name>, Option<&'static Glyph>, Option<&'static Health>);

/// What the footprint is resolved against.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Reach<'w, 's> {
    map: Res<'w, WorldMap>,
    occupancy: Res<'w, Occupancy>,
    abilities: Option<Res<'w, Abilities>>,
    offered: Option<Res<'w, Offered>>,
    bystanders: Bystanders<'w, 's>,
    users: Query<'w, 's, (&'static Position, Option<&'static Viewshed>, Option<&'static Inventory>)>,
    subjects: Query<'w, 's, Standing>,
    missiles: Query<'w, 's, (&'static Throwable, Option<&'static Name>)>,
    /// The shot the preview reaches with and whether a blow is in hand
    /// point blank: the user's own or a worn item's, as the resolver
    /// strikes.
    loadout: Loadout<'w, 's>,
    living: Query<'w, 's, (), (With<Health>, Without<Dead>)>,
    /// The odds a shot or throw lands, by the call the resolver rolls with.
    marks: Marksmanship<'w, 's>,
    /// What a worn gun or a thing used is called, for the first line of
    /// the box.
    names: Query<'w, 's, &'static Name>,
    /// What a thing does, for how its use is pointed.
    triggers: Query<'w, 's, &'static Triggers>,
    /// What is not yet known, whose reach the box leaves out.
    unknown: Query<'w, 's, (), With<Unknown>>,
    /// The accuracy the user seems to have, where a game says.
    seems: Query<'w, 's, &'static crate::view::Seems>,
}

impl Reach<'_, '_> {
    /// A row for `who` as `user` would see it under the cursor, if it is
    /// anything a row can be made of.
    fn row(&self, user: Entity, from: Point, who: Entity) -> Option<Row> {
        let (entity, at, name, glyph, health) = self.subjects.get(who).ok()?;
        let label = name.map(|n| n.as_str().to_string()).unwrap_or_default();
        // A blank glyph draws nothing and invents no colour, the same
        // choice as the empty label above.
        let glyph = glyph.copied().unwrap_or_else(|| Glyph::new(' ', Color::WHITE));
        let mut row = Row::new(entity, label, glyph).at(rl_core::geometry::chebyshev(from, at.0));
        row.health = health.map(|h| (h.current, h.max));
        row.relation = self.bystanders.relation(user, entity);
        Some(row)
    }
}

/// Fills [`TargetView`] from wherever the cursor is.
///
/// Through [`Bystanders::land`] for an ability and a thing's aimed use,
/// [`flight`] for a throw and [`shot`] for a shot, the calls the resolvers
/// act on, so the cells lit on the map, the names in the box and whether it
/// reads as refused are what will happen when the player confirms. A
/// preview computed any other way is a preview that drifts.
pub(crate) fn collect_target(mut view: ResMut<TargetView>, modals: Res<Modals>, reach: Reach) {
    // The screen is the one truth about whether the cursor is up. Closed
    // by anything but its own key, a game's action that closes every
    // screen or the close key answered by the stack, the aim is forgotten
    // here, so an overlay never stays on the map after the cursor is gone.
    if view.aiming() && !modals.is_open(target_modal(&modals)) {
        forget(&mut view);
    }
    view.cells.clear();
    view.path.clear();
    view.targets.clear();
    view.why.clear();
    view.beyond.clear();
    view.landing = None;
    view.legal = false;
    view.span = None;
    view.odds = None;
    view.seeming = false;
    view.facets.clear();
    view.unknown = false;
    let Some(user) = view.user else { return };
    let Ok((from, sight, bag)) = reach.users.get(user) else { return };
    let from = from.0;

    if view.firing {
        let Some((from_item, gun)) = reach.loadout.ranged_with(user) else { return };
        let has_melee = reach.loadout.melee(user).is_some();
        // The worn gun's name, empty for a shot that is the user's own.
        view.what = from_item.and_then(|item| reach.names.get(item).ok()).map(|n| n.as_str().to_string()).unwrap_or_default();
        view.unknown = from_item.is_some_and(|item| reach.unknown.contains(item));
        let span = AimRange { distance: rl_core::geometry::chebyshev(from, view.cursor), effective: gun.effective_range(), max: gun.range };
        view.span = (!view.unknown).then_some(span);
        let flies = shot(&reach.map, &reach.occupancy, from, view.cursor, gun.range);
        let target = mark(&reach.occupancy, user, view.cursor, |who| reach.living.contains(who));
        // Point blank is a blow, struck with whatever the user fights with
        // in hand; anything further needs a clear line to the target.
        let point_blank = rl_core::geometry::is_adjacent(from, view.cursor);
        // Reaching the cursor is the question, not resting on it: a shot
        // aimed at a wall gets there, and rests in front of it.
        let arrives = point_blank || flies.reached == Some(view.cursor);
        // The shooter's own cell, where the cursor opens with nothing in
        // sight, is nobody to shoot rather than somewhere too far.
        if view.cursor == from || (arrives && (target.is_none() || (point_blank && !has_melee))) {
            view.why.push(Blocked::NoTarget);
        } else if !arrives {
            view.why.push(Blocked::OutOfReach);
        }
        let legal = view.why.is_empty();
        view.legal = legal;
        // Point blank is struck as a blow, so its odds are the blow's, at
        // the accuracy the user seems to have.
        let shift = -reach.seems.get(user).map_or(0, |s| s.accuracy);
        view.odds = target.filter(|_| legal).and_then(|who| reach.marks.fire_odds(user, who, view.cursor, &gun, shift));
        view.seeming = shift != 0 && view.odds.is_some();
        view.targets.extend(target.filter(|_| legal).and_then(|who| reach.row(user, from, who)));
        view.cells = flies.landing.into_iter().collect();
        view.beyond = beyond(flies.landing.unwrap_or(from), view.cursor);
        view.landing = flies.landing;
        view.path = flies.path;
        return;
    }

    if let Some(item) = view.throwing {
        let Ok((throwable, name)) = reach.missiles.get(item) else { return };
        view.what = name.map(|n| n.as_str().to_string()).unwrap_or_default();
        let thrown = flight(&reach.map, &reach.occupancy, from, view.cursor, throwable.range);
        // Rolled against whoever living the flight strikes first, as the
        // resolver rolls, which need not be who the cursor is on; the range
        // read is to where it strikes, the distance the chance was worked
        // out at, so the two lines of the box agree.
        let struck = thrown.struck.filter(|who| reach.living.contains(*who));
        let reaches = match (struck, thrown.path.last()) {
            (Some(_), Some(end)) => *end,
            _ => view.cursor,
        };
        view.unknown = reach.unknown.contains(item);
        let span = AimRange { distance: rl_core::geometry::chebyshev(from, reaches), effective: throwable.effective_range(), max: throwable.range };
        view.span = (!view.unknown).then_some(span);
        // Whether a throw is rolled at all, only a thing that strikes being
        // rolled, and how far it flies without penalty are the thing's, so
        // one not yet known gives no chance, as its bag row gives no strike.
        let shift = -reach.seems.get(user).map_or(0, |s| s.accuracy);
        view.odds = struck.filter(|_| !view.unknown).and_then(|who| reach.marks.odds(user, who, Attempt::Throw(throwable), shift));
        view.seeming = shift != 0 && view.odds.is_some();
        // It lands on whoever it strikes, or wherever it comes to rest.
        let lands = match (thrown.struck, thrown.path.last()) {
            (Some(_), Some(end)) => *end,
            _ => thrown.rests,
        };
        // Refused only at the thrower's own feet, as the resolver refuses
        // it: one thrown into the wall beside them flies nowhere and is
        // still thrown, coming to rest where they stand.
        if view.cursor == from {
            view.why.push(Blocked::NoTarget);
        }
        view.legal = view.why.is_empty() && bag.is_some_and(|b| b.contains(item));
        view.targets.extend(thrown.struck.and_then(|who| reach.row(user, from, who)));
        view.cells = vec![lands];
        // A throw is not refused for falling short, so the red segment is
        // a warning here rather than a refusal: the knife lands where the
        // yellow ends.
        view.beyond = beyond(lands, view.cursor);
        view.path = thrown.path;
        view.landing = Some(lands);
        return;
    }

    let cursor = view.cursor;
    let sees_aim = sight.map(|s| s.can_see(cursor));
    if let Some(item) = view.using {
        let Some(targeting) = reach.triggers.get(item).ok().and_then(Triggers::targeting) else { return };
        view.what = reach.names.get(item).map(|n| n.as_str().to_string()).unwrap_or_default();
        // A thing no longer carried is refused, as the resolver refuses it.
        let carried = bag.is_some_and(|b| b.contains(item));
        view.unknown = reach.unknown.contains(item);
        if view.unknown {
            // Nothing its targeting would tell: a plain line to the cursor
            // and its cell, with no footprint, reach, refusal or target,
            // since who it would catch tells its shape. Taken anywhere it
            // is carried, as the resolver takes it.
            view.path = rl_core::geometry::line(from, cursor).skip(1).collect();
            view.cells = vec![cursor];
            view.legal = carried;
            return;
        }
        let source = Source::Trigger { on: item, moment: Moments::USE };
        preview(&mut view, &reach, Aimed { user, source, targeting, origin: from, aim: cursor, sees_aim }, &[]);
        view.legal = view.legal && carried;
        return;
    }

    let (Some(ability), Some(abilities)) = (view.ability, reach.abilities.as_deref()) else { return };
    let def = abilities.get(ability);
    view.what = def.name.clone();
    // What the gate said about the ability, before what this particular aim
    // adds.
    let gate = reach.offered.as_deref().map_or(&[][..], |offered| offered.why_for(user, ability));
    preview(&mut view, &reach, Aimed::ability(user, ability, def, from, cursor, sees_aim), gate);
}

/// Fills `view` from an aim the resolvers land through
/// [`Bystanders::land`], an ability's or a thing's aimed use alike: how far
/// it reaches, the footprint, who stands in it, and every reason it would
/// be refused, the gate's first and then what this particular aim adds,
/// before the turn is spent rather than after.
fn preview(view: &mut TargetView, reach: &Reach, aimed: Aimed, gate: &[Blocked]) {
    let Aimed { user, targeting, origin: from, .. } = aimed;
    let max = match targeting.mode {
        TargetMode::Own => 0,
        TargetMode::Adjacent => 1,
        TargetMode::Bolt { range } | TargetMode::Ball { range, .. } | TargetMode::Beam { range } => range,
        TargetMode::Cone { length } => length,
    };
    view.span = Some(AimRange { distance: rl_core::geometry::chebyshev(from, view.cursor), effective: max, max });
    let Landed { landing, refused } = reach.bystanders.land(aimed, &reach.map, &reach.occupancy);
    view.why.extend(gate.iter().copied());
    view.why.extend(refused);
    view.legal = view.why.is_empty();

    for who in &landing.targets {
        view.targets.extend(reach.row(user, from, *who));
    }
    // Where the aim stops short of the cursor, the rest of the way is
    // painted as out of reach: from where the projectile stopped, or from
    // the user for a shape that has no flight and reached nothing. The
    // footprint is then drawn where it was pointed rather than where it
    // stopped, since nothing lands at the stop: a refused aim shows the
    // shape the player asked for, in the tone that says no.
    let out_of_reach = view.why.contains(&Blocked::OutOfReach) || (landing.cells.is_empty() && landing.path.is_empty() && targeting.aim.needs_cursor());
    if out_of_reach {
        view.beyond = beyond(landing.landed_at.unwrap_or(from), view.cursor);
        view.cells = shape_at(targeting.mode, view.cursor);
    } else {
        view.cells = landing.cells;
    }
    view.path = landing.path;
    view.landing = landing.landed_at;
}

/// The cells `mode` would cover if it landed at `aim`: a disc for a ball,
/// the cell for anything that lands on one, nothing for a shape that
/// covers ground on the way rather than at the end.
pub(crate) fn shape_at(mode: TargetMode, aim: Point) -> Vec<Point> {
    match mode {
        TargetMode::Ball { radius, .. } => rl_core::geometry::disc(aim, radius).collect(),
        TargetMode::Bolt { .. } | TargetMode::Adjacent => vec![aim],
        TargetMode::Own | TargetMode::Beam { .. } | TargetMode::Cone { .. } => Vec::new(),
    }
}

/// The cells from `stop` to `aim`, `stop` excluded and `aim` included:
/// the part of an aim that is out of reach. Empty when the two are one.
pub fn beyond(stop: Point, aim: Point) -> Vec<Point> {
    rl_core::geometry::line(stop, aim).skip(1).collect()
}

#[cfg(test)]
pub(crate) mod harness {
    use super::*;
    use rl_rules::content::Registry;
    use rl_rules::{DamageKind, Names, StatDef};

    /// The registries an ability file names, over the one damage kind the
    /// stage has.
    pub struct Content {
        pub stats: Registry<StatDef>,
        pub kinds: Registry<DamageKind>,
    }

    impl Content {
        pub fn new() -> Self {
            Self { stats: Registry::from_defs(vec![StatDef::new("focus", 20)]).unwrap(), kinds: Registry::from_defs(vec![DamageKind::new("kinetic")]).unwrap() }
        }

        fn names(&self) -> Names<'_> {
            Names::new().stats(&self.stats).damage_kinds(&self.kinds)
        }
    }

    /// Two abilities: one that flies and bursts, one that wants no cursor.
    pub const ABILITIES: &str = r#"[
        (name: "bolt", description: "a bolt of force", mode: Bolt(range: 6), costs: [Pool("focus", 5)], effects: [(kind: "Harm", args: (kind: "kinetic", roll: "3"))]),
        (name: "burst", aim: Ground, mode: Ball(range: 6, radius: 1), effects: [(kind: "Harm", args: (kind: "kinetic", roll: "2"))]),
        (name: "steel", aim: SelfOnly, mode: Own, effects: []),
        (name: "dear", mode: Bolt(range: 6), costs: [Pool("focus", 99)], effects: []),
        (name: "salve", aim: Ally, mode: Ball(range: 6, radius: 1), effects: [(kind: "Mend", args: (roll: "2"))]),
    ]"#;

    /// Inserts everything an ability needs into `app`, before play begins.
    pub fn abilities(app: &mut App) {
        let content = Content::new();
        let built = Abilities::load(ABILITIES, app.world().resource::<EffectKinds>(), &content.names()).expect("the abilities load and build");
        app.insert_resource(built);
        app.world_mut().resource_mut::<Registries>().stats = content.stats;
    }

    /// Gives the player the abilities and something to spend on them.
    pub fn arm(stage: &mut crate::harness::Stage) -> (AbilityId, AbilityId, AbilityId) {
        let (bolt, burst, steel) = {
            let a = stage.app.world().resource::<Abilities>();
            (a.expect("bolt"), a.expect("burst"), a.expect("steel"))
        };
        let dear = stage.app.world().resource::<Abilities>().expect("dear");
        let mut pools = Pools::new();
        pools.set(rl_rules::StatId::from_raw(0), 20);
        let player = stage.player;
        stage.app.world_mut().entity_mut(player).insert((Grants(vec![bolt, burst, steel, dear]), pools));
        stage.tick();
        stage.tick();
        (bolt, burst, steel)
    }
}

#[cfg(test)]
mod tests {
    use super::harness::{abilities, arm};
    use super::*;
    use crate::harness::Stage;
    use crate::view::Seems;
    use rl_bevy::AddEngineEffects;

    fn staged() -> Stage {
        let mut stage = Stage::new_with((AbilitiesPlugin, TargetViewPlugin), |app| {
            app.add_engine_effects();
            abilities(app);
        });
        stage.tick();
        stage
    }

    fn intents(stage: &mut Stage) -> Vec<Use> {
        stage.app.world_mut().resource_mut::<Messages<Intent<Use>>>().drain().map(|i| i.action).collect()
    }

    fn aim_at(stage: &mut Stage, ability: AbilityId) {
        let user = stage.player;
        stage.app.world_mut().write_message(AimAt { user, ability });
        stage.tick();
    }

    /// The player's own side, for an actor that should read as an ally.
    fn befriend(stage: &mut Stage, who: Entity) {
        let ours = stage.app.world().get::<Faction>(stage.player).expect("the player takes a side").0;
        stage.app.world_mut().entity_mut(who).insert(Faction(ours));
    }

    /// The cursor opens on the nearest thing the ability wants, which is
    /// the same choice a mind's tactic would have made.
    #[test]
    fn the_cursor_opens_on_the_nearest_thing_the_aim_wants() {
        let mut stage = staged();
        let (bolt, _, _) = arm(&mut stage);
        stage.actor("far one", 'f', 4, 0);
        stage.actor("near one", 'n', 2, 0);
        stage.tick();

        aim_at(&mut stage, bolt);
        let at = stage.at;
        let view = stage.app.world().resource::<TargetView>();
        assert_eq!(view.cursor, at.offset(2, 0), "the nearer of the two");
        assert!(view.aiming());
        assert!(stage.app.world().resource::<Modals>().any_open(), "and it holds input");
    }

    /// Cycling walks what the ability wants, and a direction key steps
    /// off it a cell at a time.
    #[test]
    fn tab_cycles_the_targets_and_a_direction_key_steps() {
        let mut stage = staged();
        let (bolt, _, _) = arm(&mut stage);
        stage.actor("far one", 'f', 4, 0);
        stage.actor("near one", 'n', 2, 0);
        stage.tick();
        aim_at(&mut stage, bolt);
        let at = stage.at;

        stage.press(KeyCode::Tab);
        assert_eq!(stage.app.world().resource::<TargetView>().cursor, at.offset(4, 0));
        stage.press(KeyCode::Tab);
        assert_eq!(stage.app.world().resource::<TargetView>().cursor, at.offset(2, 0), "round the end");
        stage.press(KeyCode::ArrowUp);
        assert_eq!(stage.app.world().resource::<TargetView>().cursor, at.offset(2, -1), "stepped off the target");
    }

    /// An ability aimed at foes stops only on foes, however much else is in
    /// sight.
    #[test]
    fn an_aim_at_foes_cycles_past_allies_and_things() {
        let mut stage = staged();
        let (bolt, _, _) = arm(&mut stage);
        let friend = stage.actor("friend", 'a', 1, 0);
        befriend(&mut stage, friend);
        stage.thing("a coin", '$', 1, 1);
        stage.actor("foe", 'f', 3, 0);
        stage.tick();
        aim_at(&mut stage, bolt);
        let at = stage.at;
        assert_eq!(stage.app.world().resource::<TargetView>().cursor, at.offset(3, 0));
        stage.press(KeyCode::Tab);
        assert_eq!(stage.app.world().resource::<TargetView>().cursor, at.offset(3, 0), "the foe is the only stop");
    }

    /// An ability aimed at the ground has no cells to cycle through, so it
    /// cycles through what is in sight instead, things included, in the
    /// order the nearby list prints them.
    #[test]
    fn an_aim_at_the_ground_cycles_through_everything_in_sight() {
        let mut stage = staged();
        let (_, burst, _) = arm(&mut stage);
        let friend = stage.actor("friend", 'a', -2, 0);
        befriend(&mut stage, friend);
        stage.actor("them", 't', 3, 0);
        stage.thing("a coin", '$', 1, 0);
        stage.tick();
        aim_at(&mut stage, burst);
        let at = stage.at;
        let cursor = |stage: &Stage| stage.app.world().resource::<TargetView>().cursor;
        assert_eq!(cursor(&stage), at.offset(3, 0), "it opens on the foe worth bursting");
        stage.press(KeyCode::Tab);
        assert_eq!(cursor(&stage), at.offset(1, 0), "then the coin, after every actor");
        stage.press(KeyCode::Tab);
        assert_eq!(cursor(&stage), at.offset(-2, 0), "then round to the ally, the nearest actor");
        stage.press(KeyCode::Tab);
        assert_eq!(cursor(&stage), at.offset(3, 0));
    }

    /// Something picked out before the cursor opened is where it opens,
    /// when the aim can take it, and putting the cursor away leaves nothing
    /// picked out.
    #[test]
    fn the_cursor_opens_on_what_is_picked_out_when_the_aim_can_take_it() {
        let mut stage = staged();
        let (bolt, _, _) = arm(&mut stage);
        stage.actor("near one", 'n', 2, 0);
        let far = stage.actor("far one", 'f', 4, 0);
        let coin = stage.thing("a coin", '$', 1, 0);
        stage.tick();
        let at = stage.at;

        stage.app.world_mut().resource_mut::<Focus>().set(Some(far));
        aim_at(&mut stage, bolt);
        assert_eq!(stage.app.world().resource::<TargetView>().cursor, at.offset(4, 0), "on the one picked out, not the nearest");
        stage.press(KeyCode::Escape);

        stage.app.world_mut().resource_mut::<Focus>().set(Some(coin));
        aim_at(&mut stage, bolt);
        assert_eq!(stage.app.world().resource::<TargetView>().cursor, at.offset(2, 0), "a bolt at foes cannot take a coin");
        stage.press(KeyCode::Escape);
        assert_ne!(stage.app.world().resource::<Focus>().get(), Some(coin), "the cursor moved the focus to what it opened on");
    }

    /// The aim remembers what it was last confirmed at and opens on it
    /// again, the nearest notwithstanding, without leaving it picked out in
    /// the nearby list: once the cursor is put away, fired or not, nothing
    /// is picked out, and a cancelled aim does not change what is
    /// remembered.
    #[test]
    fn the_next_aim_opens_on_the_last_one_confirmed_and_nothing_stays_picked_out() {
        let mut stage = staged();
        let (bolt, _, _) = arm(&mut stage);
        stage.actor("near one", 'n', 2, 0);
        let far = stage.actor("far one", 'f', 4, 0);
        stage.tick();
        let at = stage.at;

        aim_at(&mut stage, bolt);
        assert_eq!(stage.app.world().resource::<TargetView>().cursor, at.offset(2, 0), "the nearest, the first time");
        stage.press(KeyCode::Tab);
        assert_eq!(stage.app.world().resource::<Focus>().get(), Some(far), "the list follows the cursor while it aims");
        stage.press(KeyCode::Enter);
        assert_eq!(stage.app.world().resource::<Focus>().get(), None, "fired, and nothing is left picked out");

        aim_at(&mut stage, bolt);
        assert_eq!(stage.app.world().resource::<TargetView>().cursor, at.offset(4, 0), "back on the one fired at, not the nearest");
        stage.press(KeyCode::Tab);
        stage.press(KeyCode::Escape);
        assert_eq!(stage.app.world().resource::<Focus>().get(), None, "put away, and nothing is left picked out");

        aim_at(&mut stage, bolt);
        assert_eq!(stage.app.world().resource::<TargetView>().cursor, at.offset(4, 0), "a cancelled aim is not remembered over a fired one");
    }

    /// What the cursor shows is what the resolver will do: the same
    /// footprint call, so a burst covers the disc it will burst into.
    #[test]
    fn the_footprint_shown_is_the_footprint_that_will_land() {
        let mut stage = staged();
        let (_, burst, _) = arm(&mut stage);
        stage.actor("them", 't', 3, 0);
        stage.tick();
        aim_at(&mut stage, burst);
        let at = stage.at;

        let view = stage.app.world().resource::<TargetView>();
        let burst_at = at.offset(3, 0);
        assert_eq!(view.cursor, burst_at);
        // A disc of one is the five-cell plus the geometry draws, and the
        // point of resolving it here is that this is that exact call.
        let expected = rl_core::geometry::disc(burst_at, 1).collect::<Vec<_>>();
        let mut got = view.cells.clone();
        got.sort_by_key(|p| (p.y, p.x));
        let mut want = expected;
        want.sort_by_key(|p| (p.y, p.x));
        assert_eq!(got, want, "the shape the resolver will burst into");
        assert_eq!(view.path.last(), Some(&burst_at), "the flight ends where it bursts");
        assert_eq!(view.targets.len(), 1);
        assert_eq!(view.targets[0].label, "them");
        assert!(view.legal);
    }

    /// A shot, a throw and a bolt are one projectile, so sent at the same
    /// wall all three come to rest on the same cell, the open one in front
    /// of it, and none flies into it: what each preview marks is what its
    /// resolver lands, through the one call all three are answered by.
    #[test]
    fn a_shot_a_throw_and_a_bolt_aimed_at_a_wall_all_come_to_rest_short_of_it() {
        let mut stage = staged();
        let (bolt, _, _) = arm(&mut stage);
        let (user, kind, at) = (stage.player, stage.kind, stage.at);
        let knife = stage.app.world_mut().spawn((Item, Name::new("knife"), Throwable::new(6, Some((kind, rl_core::DiceRoll::flat(2)))))).id();
        stage.app.world_mut().get_mut::<Inventory>(user).expect("a bag").items.push(knife);
        stage.app.world_mut().entity_mut(user).insert(RangedAttack::new(kind, rl_core::DiceRoll::flat(2), 6));
        let wall = at.offset(3, 0);
        rl_bevy::testing::wall_off(&mut stage.app, &[wall]);
        stage.tick();
        let mut at_the_wall = |open: &dyn Fn(&mut World)| {
            open(stage.app.world_mut());
            stage.tick();
            stage.app.world_mut().resource_mut::<TargetView>().cursor = wall;
            stage.tick();
            let view = stage.app.world().resource::<TargetView>();
            let seen = (view.landing, view.path.clone());
            stage.press(KeyCode::Escape);
            seen
        };
        let shot = at_the_wall(&|world| {
            world.write_message(AimFire { user });
        });
        let thrown = at_the_wall(&|world| {
            world.write_message(AimThrow { user, item: knife });
        });
        let bolted = at_the_wall(&|world| {
            world.write_message(AimAt { user, ability: bolt });
        });
        let short = at.offset(2, 0);
        let rests = (Some(short), vec![at.offset(1, 0), short]);
        assert_eq!(shot, rests, "a shot rests in front of the wall");
        assert_eq!(thrown, rests, "a throw rests where the shot does");
        assert_eq!(bolted, rests, "and so does a bolt");
    }

    /// An aim the resolver would refuse says so before the turn is spent,
    /// and says why.
    #[test]
    fn an_aim_that_would_be_refused_reads_as_illegal_with_its_reason() {
        let mut stage = staged();
        let (bolt, _, _) = arm(&mut stage);
        stage.actor("them", 't', 2, 0);
        stage.tick();

        // Out of the ability's range: the bolt lands short, so nothing is
        // under the cursor.
        aim_at(&mut stage, bolt);
        let at = stage.at;
        stage.app.world_mut().resource_mut::<TargetView>().cursor = at.offset(2, -3);
        stage.tick();
        let view = stage.app.world().resource::<TargetView>();
        assert!(view.legal, "an empty cell in reach is still a legal aim for a bolt");

        // The ability nobody can pay for: refused before it is pointed
        // anywhere, from the same gate the resolver runs.
        let dear = stage.app.world().resource::<Abilities>().expect("dear");
        aim_at(&mut stage, dear);
        stage.tick();
        let view = stage.app.world().resource::<TargetView>();
        assert!(!view.legal, "it cannot be paid for");
        assert!(view.why.iter().any(|w| matches!(w, Blocked::Cannot(_))), "{:?}", view.why);
    }

    /// The property the preview exists for, over many layouts: who the
    /// box lists and whether it reads as refused are exactly what the
    /// resolver does once the player confirms.
    #[test]
    fn the_preview_names_exactly_who_the_resolver_hits() {
        use rl_core::seed::position_hash;
        for seed in 0..12u64 {
            // A small deterministic stream: one hash per draw.
            let mut draw = 0;
            let mut roll = |n: i32| {
                draw += 1;
                (position_hash(seed, draw, n) % n as u64) as i32
            };
            let mut stage = staged();
            let (bolt, burst, _) = arm(&mut stage);
            let salve = stage.app.world().resource::<Abilities>().expect("salve");
            let player = stage.player;
            stage.app.world_mut().get_mut::<Grants>(player).expect("grants").0.push(salve);
            if roll(2) == 0 {
                stage.app.world_mut().get_mut::<Health>(player).expect("health").current = 12;
            }
            let mut taken = vec![(0, 0)];
            for i in 0..6 {
                let (dx, dy) = loop {
                    let at = (roll(9) - 4, roll(9) - 4);
                    if !taken.contains(&at) {
                        break at;
                    }
                };
                taken.push((dx, dy));
                let who = stage.actor(&format!("them {i}"), 't', dx, dy);
                if roll(2) == 0 {
                    stage.app.world_mut().entity_mut(who).insert(Faction(rl_rules::FactionId::from_raw(0)));
                }
                if roll(2) == 0 {
                    stage.app.world_mut().get_mut::<Health>(who).expect("health").current = 4;
                }
            }
            stage.tick();
            stage.tick();

            for _ in 0..6 {
                let ability = [bolt, burst, salve][roll(3) as usize];
                let aim = stage.at.offset(roll(13) - 6, roll(13) - 6);
                let user = stage.player;
                stage.app.world_mut().write_message(AimAt { user, ability });
                stage.tick();
                stage.app.world_mut().resource_mut::<TargetView>().cursor = aim;
                stage.tick();
                let view = stage.app.world().resource::<TargetView>();
                assert!(view.aiming(), "seed {seed}: the cursor is up");
                let legal = view.legal;
                let mut shown: Vec<Entity> = view.targets.iter().map(|r| r.entity).collect();
                shown.sort();

                let _ = stage.app.world_mut().resource_mut::<Messages<AbilityEvent>>().drain().count();
                stage.press(KeyCode::Enter);
                let outcomes: Vec<AbilityEvent> = stage.app.world_mut().resource_mut::<Messages<AbilityEvent>>().drain().collect();
                match outcomes.as_slice() {
                    [AbilityEvent::Used { targets, .. }] => {
                        let mut hit = targets.clone();
                        hit.sort();
                        assert!(legal, "seed {seed}: the preview refused an aim the resolver took");
                        assert_eq!(hit, shown, "seed {seed}: the box listed other than who was hit");
                    }
                    [AbilityEvent::Refused { why, .. }] => assert!(!legal, "seed {seed}: the resolver refused ({why:?}) an aim the preview called legal"),
                    other => panic!("seed {seed}: expected one outcome, got {other:?}"),
                }
            }
        }
    }

    /// Confirming spends the turn on the aim; cancelling spends nothing.
    #[test]
    fn confirm_writes_the_use_and_cancel_writes_nothing() {
        let mut stage = staged();
        let (bolt, _, _) = arm(&mut stage);
        stage.actor("them", 't', 2, 0);
        stage.tick();
        let at = stage.at;

        aim_at(&mut stage, bolt);
        let _ = intents(&mut stage);
        stage.press(KeyCode::Escape);
        assert!(intents(&mut stage).is_empty(), "cancelled");
        assert!(!stage.app.world().resource::<TargetView>().aiming());
        assert!(!stage.app.world().resource::<Modals>().any_open(), "and input came back");

        aim_at(&mut stage, bolt);
        let _ = intents(&mut stage);
        stage.press(KeyCode::Enter);
        assert_eq!(intents(&mut stage), vec![Use { ability: bolt, aim: at.offset(2, 0) }]);
        assert!(!stage.app.world().resource::<TargetView>().aiming(), "and the cursor went away");
    }

    /// A throw is aimed like an ability: the cursor opens on the nearest
    /// foe, names who the flight strikes first, and confirming throws it.
    #[test]
    fn a_throw_is_aimed_at_the_nearest_foe_previews_who_it_strikes_and_confirm_throws_it() {
        let mut stage = Stage::new_with((ThrowingPlugin, TargetViewPlugin), |_| {});
        stage.tick();
        let (user, kind) = (stage.player, stage.kind);
        let knife = stage.app.world_mut().spawn((Item, Name::new("knife"), Throwable::new(6, Some((kind, rl_core::DiceRoll::flat(2)))))).id();
        stage.app.world_mut().get_mut::<Inventory>(user).expect("a bag").items.push(knife);
        stage.actor("near", 'n', 2, 0);
        stage.actor("far", 'f', 4, 0);
        stage.tick();
        let at = stage.at;

        stage.app.world_mut().write_message(AimThrow { user, item: knife });
        stage.tick();
        let view = stage.app.world().resource::<TargetView>();
        assert!(view.aiming() && view.throwing == Some(knife));
        assert_eq!(view.cursor, at.offset(2, 0), "the nearest foe");
        assert_eq!(view.what, "knife");
        stage.press(KeyCode::Tab);
        let view = stage.app.world().resource::<TargetView>();
        assert_eq!(view.cursor, at.offset(4, 0), "aimed past the near one");
        assert_eq!(view.targets.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(), vec!["near"], "which the flight strikes first");
        assert_eq!(view.landing, Some(at.offset(2, 0)));
        assert!(view.legal);

        let _ = stage.app.world_mut().resource_mut::<Messages<Intent<Throw>>>().drain().count();
        stage.press(KeyCode::Enter);
        let thrown: Vec<Throw> = stage.app.world_mut().resource_mut::<Messages<Intent<Throw>>>().drain().map(|i| i.action).collect();
        assert_eq!(thrown, vec![Throw { item: knife, at: at.offset(4, 0) }], "confirm throws it where it was aimed");
        assert!(!stage.app.world().resource::<TargetView>().aiming());
    }

    fn percent(app: &mut App) {
        app.insert_resource(HitRules(Box::new(rl_rules::Percent::new(5, 16, 30))));
    }

    #[test]
    fn a_shot_carries_its_range_and_the_resolvers_odds() {
        let mut stage = Stage::new_with(TargetViewPlugin, percent);
        let (user, kind) = (stage.player, stage.kind);
        stage.app.world_mut().entity_mut(user).insert(RangedAttack::new(kind, rl_core::DiceRoll::flat(2), 12).effective_to(3));
        stage.actor("droid", 'd', 5, 0);
        stage.tick();
        stage.app.world_mut().write_message(AimFire { user });
        stage.tick();
        let view = stage.app.world().resource::<TargetView>();
        assert_eq!(view.span, Some(AimRange { distance: 5, effective: 3, max: 12 }));
        assert_eq!(view.odds.as_ref().map(rl_rules::Odds::percent), Some(90));
    }

    /// The chance of a shot at the accuracy the shooter seems to have: with
    /// an accuracy stat named, a `Seems` taking ten off reads ten less and
    /// says it is seeming; with none it is the true chance.
    #[test]
    fn a_shot_is_reckoned_at_the_accuracy_the_shooters_seems_leaves_and_says_so() {
        let aimed = |seems: Option<Seems>| {
            let mut stage = Stage::new_with(TargetViewPlugin, |app| {
                percent(app);
                let stats = rl_rules::Registry::from_defs(vec![rl_rules::StatDef::new("accuracy", 100)]).expect("one stat");
                let stat = stats.expect("accuracy");
                app.world_mut().resource_mut::<Registries>().stats = stats;
                app.world_mut().resource_mut::<rl_bevy::CombatRules>().accuracy = Some(stat);
            });
            let (user, kind) = (stage.player, stage.kind);
            stage
                .app
                .world_mut()
                .entity_mut(user)
                .insert((RangedAttack::new(kind, rl_core::DiceRoll::flat(2), 12).effective_to(3), StatBlock(rl_rules::Stats::new())));
            if let Some(seems) = seems {
                stage.app.world_mut().entity_mut(user).insert(seems);
            }
            stage.actor("droid", 'd', 3, 0);
            stage.tick();
            stage.app.world_mut().write_message(AimFire { user });
            stage.tick();
            let view = stage.app.world().resource::<TargetView>();
            (view.odds.as_ref().map(rl_rules::Odds::percent), view.seeming)
        };
        assert_eq!(aimed(None), (Some(100), false), "the true chance");
        assert_eq!(aimed(Some(Seems { accuracy: 10, ..Default::default() })), (Some(90), true), "ten less, as it seems");
        assert_eq!(aimed(Some(Seems::default())), (Some(100), false), "nothing taken off, nothing said");
    }

    #[test]
    fn point_blank_is_a_blow_and_carries_the_blows_odds() {
        let mut stage = Stage::new_with(TargetViewPlugin, percent);
        let (user, kind) = (stage.player, stage.kind);
        stage.app.world_mut().entity_mut(user).insert(RangedAttack::new(kind, rl_core::DiceRoll::flat(2), 12).effective_to(0));
        stage.actor("rat", 'r', 1, 0);
        stage.tick();
        stage.app.world_mut().write_message(AimFire { user });
        stage.tick();
        let odds = stage.app.world().resource::<TargetView>().odds.clone().expect("the player has a fist");
        assert_eq!((odds.percent(), odds.lines.len()), (100, 0), "a blow reads no range and no light");
    }

    /// With nothing in sight the cursor opens on the shooter's own cell, and
    /// that is no target, not a target out of reach: nothing is too far.
    #[test]
    fn a_shot_aimed_at_the_shooters_own_cell_has_no_target_rather_than_being_out_of_reach() {
        let mut stage = Stage::new_with(TargetViewPlugin, |_| {});
        let (user, kind) = (stage.player, stage.kind);
        stage.app.world_mut().entity_mut(user).insert(RangedAttack::new(kind, rl_core::DiceRoll::flat(2), 12));
        stage.tick();
        stage.app.world_mut().write_message(AimFire { user });
        stage.tick();
        let view = stage.app.world().resource::<TargetView>();
        assert_eq!(view.cursor, stage.at, "nothing in sight, so it opens on the shooter");
        assert_eq!(view.why, vec![Blocked::NoTarget]);
        assert!(view.beyond.is_empty(), "and nothing is painted as out of reach");
    }

    #[test]
    fn an_empty_cell_has_a_span_and_no_odds() {
        let mut stage = Stage::new_with(TargetViewPlugin, percent);
        let (user, kind) = (stage.player, stage.kind);
        stage.app.world_mut().entity_mut(user).insert(RangedAttack::new(kind, rl_core::DiceRoll::flat(2), 12));
        stage.tick();
        stage.app.world_mut().write_message(AimFire { user });
        stage.tick();
        let at = stage.at;
        stage.app.world_mut().resource_mut::<TargetView>().cursor = at.offset(0, 4);
        stage.tick();
        let view = stage.app.world().resource::<TargetView>();
        assert_eq!(view.odds, None);
        assert_eq!(view.span.map(|s| s.distance), Some(4));
    }

    #[test]
    fn a_throw_is_rolled_against_the_body_it_strikes_first_at_that_bodys_distance() {
        let mut stage = Stage::new_with(TargetViewPlugin, percent);
        let (user, kind) = (stage.player, stage.kind);
        let knife = stage.app.world_mut().spawn((Item, Name::new("knife"), Throwable::new(9, Some((kind, rl_core::DiceRoll::flat(2)))).effective_to(1))).id();
        stage.app.world_mut().get_mut::<Inventory>(user).expect("a bag").items.push(knife);
        stage.actor("near", 'n', 2, 0);
        stage.actor("far", 'f', 6, 0);
        stage.tick();
        stage.app.world_mut().write_message(AimThrow { user, item: knife });
        stage.tick();
        let at = stage.at;
        stage.app.world_mut().resource_mut::<TargetView>().cursor = at.offset(6, 0);
        stage.tick();
        let view = stage.app.world().resource::<TargetView>();
        assert_eq!(view.odds.as_ref().map(rl_rules::Odds::percent), Some(95), "one tile past effective, against the near body");
        assert_eq!(view.span.map(|s| s.distance), Some(2), "and the range read is the near body's, which the chance was worked out at");
    }

    /// Something in the way that nothing can hurt, a crate with no health,
    /// stops the throw and is never rolled against, so no chance is shown.
    #[test]
    fn a_throw_stopped_by_something_that_cannot_be_hurt_shows_no_odds() {
        let mut stage = Stage::new_with(TargetViewPlugin, percent);
        let (user, kind) = (stage.player, stage.kind);
        let knife = stage.app.world_mut().spawn((Item, Name::new("knife"), Throwable::new(9, Some((kind, rl_core::DiceRoll::flat(2)))))).id();
        stage.app.world_mut().get_mut::<Inventory>(user).expect("a bag").items.push(knife);
        let at = stage.at;
        stage.app.world_mut().spawn((Blocks, Position(at.offset(2, 0)), Name::new("crate")));
        stage.actor("far", 'f', 6, 0);
        stage.tick();
        stage.app.world_mut().write_message(AimThrow { user, item: knife });
        stage.tick();
        stage.app.world_mut().resource_mut::<TargetView>().cursor = at.offset(6, 0);
        stage.tick();
        assert_eq!(stage.app.world().resource::<TargetView>().odds, None);
    }

    #[test]
    fn a_grenade_has_no_odds() {
        let mut stage = Stage::new_with(TargetViewPlugin, percent);
        let user = stage.player;
        let grenade = stage.app.world_mut().spawn((Item, Name::new("grenade"), Throwable::new(6, None))).id();
        stage.app.world_mut().get_mut::<Inventory>(user).expect("a bag").items.push(grenade);
        stage.actor("droid", 'd', 3, 0);
        stage.tick();
        stage.app.world_mut().write_message(AimThrow { user, item: grenade });
        stage.tick();
        let view = stage.app.world().resource::<TargetView>();
        assert!(view.aiming());
        assert_eq!(view.odds, None);
    }

    /// A thrown thing not yet known is previewed as a known one is where it
    /// shows only what its thrower knows, the flight and the cell it comes
    /// down on, and never the area its landing covers; but it gives no
    /// chance to hit, since only a thing that strikes is rolled, and what it
    /// strikes for is as hidden as on its bag row.
    #[test]
    fn an_unknown_thrown_thing_paints_its_flight_and_landing_cell_and_gives_no_chance_to_hit() {
        let mut stage = Stage::new_with(TargetViewPlugin, percent);
        let (user, kind) = (stage.player, stage.kind);
        let knife = || (Item, Name::new("knife"), Throwable::new(6, Some((kind, rl_core::DiceRoll::flat(2)))));
        let (known, unknown) = (stage.app.world_mut().spawn(knife()).id(), stage.app.world_mut().spawn((knife(), rl_bevy::Unknown)).id());
        stage.app.world_mut().get_mut::<Inventory>(user).expect("a bag").items.extend([known, unknown]);
        stage.actor("droid", 'd', 3, 0);
        stage.tick();
        let at = stage.at;
        let mut preview = |item: Entity| {
            stage.app.world_mut().write_message(AimThrow { user, item });
            stage.tick();
            stage.app.world_mut().resource_mut::<TargetView>().cursor = at.offset(3, 0);
            stage.tick();
            let view = stage.app.world().resource::<TargetView>();
            let seen = (view.path.clone(), view.cells.clone(), view.beyond.clone(), view.odds.is_some(), view.unknown);
            stage.press(KeyCode::Escape);
            seen
        };
        let (shown, hidden) = (preview(known), preview(unknown));
        assert_eq!((&hidden.0, &hidden.1, &hidden.2), (&shown.0, &shown.1, &shown.2), "the same flight, landing cell and warning");
        assert_eq!(hidden.1, vec![at.offset(3, 0)], "the cell it comes down on, and no area");
        assert!(shown.3 && !hidden.3, "a chance to hit for the known knife, none for the unknown one");
        assert!(hidden.4, "which is said to be unknown");
    }

    /// A shot is aimed like a throw, and the preview is the resolver's own
    /// line of fire: a foe behind another reads as out of reach, and
    /// confirming writes the attack on whoever is under the cursor.
    #[test]
    fn a_shot_opens_on_the_nearest_foe_previews_its_line_and_confirm_attacks_who_is_under_the_cursor() {
        let mut stage = Stage::new_with(TargetViewPlugin, |_| {});
        stage.tick();
        let (user, kind) = (stage.player, stage.kind);
        stage.app.world_mut().entity_mut(user).insert(RangedAttack::new(kind, rl_core::DiceRoll::flat(2), 6));
        let near = stage.actor("near", 'n', 2, 0);
        let far = stage.actor("far", 'f', 4, 0);
        stage.tick();
        let at = stage.at;
        let attacks = |stage: &mut Stage| stage.app.world_mut().resource_mut::<Messages<Intent<Attack>>>().drain().map(|i| i.action.0).collect::<Vec<_>>();

        stage.app.world_mut().write_message(AimFire { user });
        stage.tick();
        let view = stage.app.world().resource::<TargetView>();
        assert!(view.aiming() && view.firing);
        assert_eq!(view.cursor, at.offset(2, 0), "the nearest foe");
        assert!(view.legal);
        assert_eq!(view.targets.iter().map(|r| r.entity).collect::<Vec<_>>(), vec![near]);

        stage.press(KeyCode::Tab);
        let view = stage.app.world().resource::<TargetView>();
        assert_eq!(view.cursor, at.offset(4, 0));
        assert!(!view.legal, "the near one stands in the line of fire");
        assert_eq!(view.why, vec![Blocked::OutOfReach]);
        assert_eq!(view.landing, Some(at.offset(2, 0)), "and the shot would stop at them");
        assert_eq!(view.beyond, vec![at.offset(3, 0), at.offset(4, 0)], "the rest of the way is out of reach");

        stage.press(KeyCode::ArrowUp);
        let _ = attacks(&mut stage);
        stage.press(KeyCode::Enter);
        assert!(attacks(&mut stage).is_empty(), "nobody under the cursor, so nothing is shot at");
        assert!(stage.app.world().resource::<TargetView>().aiming(), "and the cursor stays up");

        stage.press(KeyCode::ArrowDown);
        stage.press(KeyCode::Enter);
        assert_eq!(attacks(&mut stage), vec![far], "the resolver judges the blocked line, as it would any attack");
        assert!(!stage.app.world().resource::<TargetView>().aiming());
    }

    /// A gun worn is a gun to aim: the cursor opens for a user whose only
    /// shot comes from what it wears, and the preview reaches as far as
    /// that gun does, because both read the same [`Loadout`] the resolver
    /// fires with.
    #[test]
    fn a_shot_from_a_worn_gun_opens_the_cursor_and_previews_the_guns_own_reach() {
        let mut stage = Stage::new_with(TargetViewPlugin, |_| {});
        let (user, kind) = (stage.player, stage.kind);
        let gun = stage.app.world_mut().spawn((Item, RangedAttack::new(kind, rl_core::DiceRoll::flat(2), 3))).id();
        let mut worn = Equipped(rl_rules::Equipment::with_slot_count(1));
        worn.equip(gun, &rl_rules::EquipShape::in_slot(rl_rules::SlotId::from_raw(0))).expect("one slot, one gun");
        stage.app.world_mut().entity_mut(user).insert(worn);
        stage.actor("near", 'n', 2, 0);
        stage.actor("far", 'f', 5, 0);
        stage.tick();
        let at = stage.at;

        stage.app.world_mut().write_message(AimFire { user });
        stage.tick();
        let view = stage.app.world().resource::<TargetView>();
        assert!(view.aiming() && view.firing, "the worn gun opened the cursor");
        assert_eq!(view.cursor, at.offset(2, 0), "on the nearest foe");
        assert!(view.legal, "within the gun's three tiles");

        stage.press(KeyCode::Tab);
        let view = stage.app.world().resource::<TargetView>();
        assert_eq!(view.cursor, at.offset(5, 0));
        assert_eq!(view.why, vec![Blocked::OutOfReach], "five tiles is past a gun that reaches three");
    }

    /// A user with nothing to shoot with opens no cursor.
    #[test]
    fn a_shot_without_a_ranged_attack_opens_nothing() {
        let mut stage = Stage::new_with(TargetViewPlugin, |_| {});
        let user = stage.player;
        stage.actor("them", 't', 2, 0);
        stage.tick();
        stage.app.world_mut().write_message(AimFire { user });
        stage.tick();
        assert!(!stage.app.world().resource::<TargetView>().aiming());
        assert!(!stage.app.world().resource::<Modals>().any_open());
    }

    /// An ability that wants no cursor is used where it stands, so a game
    /// binds every ability the same way.
    #[test]
    fn an_ability_that_needs_no_cursor_opens_no_screen() {
        let mut stage = staged();
        let (_, _, steel) = arm(&mut stage);
        let at = stage.at;

        aim_at(&mut stage, steel);
        assert_eq!(intents(&mut stage), vec![Use { ability: steel, aim: at }]);
        assert!(!stage.app.world().resource::<Modals>().any_open(), "nothing opened");
        assert!(!stage.app.world().resource::<TargetView>().aiming());
    }

    /// However the screen goes away, the aim goes with it: the close key,
    /// answered by the cursor or by the stack for it, and a game closing
    /// every screen at once, all leave nothing drawn on the map.
    #[test]
    fn the_aim_is_forgotten_whenever_its_screen_is_gone() {
        let mut stage = staged();
        let (bolt, _, _) = arm(&mut stage);
        stage.actor("them", 't', 2, 0);
        stage.tick();
        let user = stage.player;
        let aim = |stage: &mut Stage| {
            stage.app.world_mut().write_message(AimAt { user, ability: bolt });
            stage.tick();
            assert!(stage.app.world().resource::<TargetView>().aiming(), "up");
        };

        aim(&mut stage);
        stage.press(CursorKeys::default().close);
        assert!(!stage.app.world().resource::<TargetView>().aiming(), "the close key puts the aim away");
        assert_eq!(stage.app.world().resource::<Modals>().top(), None, "and only the one screen went");

        aim(&mut stage);
        stage.app.world_mut().resource_mut::<Modals>().close_all();
        stage.tick();
        assert!(!stage.app.world().resource::<TargetView>().aiming(), "closed from outside, the aim is still forgotten");
        assert!(stage.app.world().resource::<TargetView>().cells.is_empty(), "and nothing is left to draw");
    }

    /// Every ability known, in a stable order, with the reasons the gate
    /// gave for the ones that cannot be used.
    #[test]
    fn the_menu_lists_what_is_known_and_why_it_cannot_be_used() {
        use crate::view::ability::{AbilityView, AbilityViewPlugin};
        let mut stage = Stage::new_with((AbilitiesPlugin, AbilityViewPlugin), |app| {
            app.add_engine_effects();
            abilities(app);
        });
        stage.tick();
        arm(&mut stage);
        stage.tick();

        let view = stage.app.world().resource::<AbilityView>();
        let names: Vec<&str> = view.rows.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(names, vec!["bolt", "burst", "steel", "dear"], "registration order, so a bound key stays bound");
        assert_eq!(view.ready(), 3);
        let dear = view.rows.iter().find(|r| r.label == "dear").expect("the dear one");
        assert!(!dear.ready());
        assert!(matches!(dear.blocked.first(), Some(Blocked::Cannot(_))), "{:?}", dear.blocked);
    }
}

/// A thing used at somewhere, aimed through the same cursor an ability is.
#[cfg(test)]
mod uses {
    use super::*;
    use crate::harness::Stage;
    use rl_bevy::{AddEffect, AddEngineEffects, EffectWorld, EffectsPlugin, FromArgs, Landing, Moments, Triggers};
    use rl_rules::Names;
    use rl_rules::ability::RawValue;

    /// A rod whose use bursts on the ground around where it is pointed.
    const BURST: &str = r#"[(on: "use", targeting: (mode: Ball(range: 6, radius: 1), aim: Ground), effects: [(kind: "Struck")])]"#;

    /// Whoever a use landed on, marked by the effect itself, so what is
    /// read is the landing rather than something it went on to cause.
    #[derive(Component)]
    struct Hit;

    /// The effect that marks them.
    struct Struck;

    impl Effect for Struck {
        fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
            for who in &landing.targets {
                world.commands.entity(*who).insert(Hit);
            }
        }
    }

    impl FromArgs for Struck {
        const KIND: &'static str = "Struck";

        fn from_args(_: &RawValue, _: &Names<'_>) -> Result<Self, String> {
            Ok(Self)
        }
    }

    fn staged() -> Stage {
        let mut stage = Stage::new_with((EffectsPlugin, TargetViewPlugin), |app| {
            app.add_engine_effects().add_effect::<Struck>();
        });
        stage.tick();
        stage
    }

    /// A thing called `name` in the player's bag, doing `triggers`.
    fn carry(stage: &mut Stage, name: &str, triggers: &str) -> Entity {
        let specs: Vec<rl_rules::TriggerSpec> = Names::new().load_list(triggers).expect("the triggers parse");
        let built = {
            let world = stage.app.world();
            let names = world.resource::<Registries>().names();
            Triggers::build(&specs, &[], world.resource::<Moments>(), world.resource::<EffectKinds>(), &names).expect("the triggers build")
        };
        let thing = stage.app.world_mut().spawn((Item, Name::new(name.to_string()), built)).id();
        let player = stage.player;
        stage.app.world_mut().get_mut::<Inventory>(player).expect("a bag").items.push(thing);
        thing
    }

    fn aimed_uses(stage: &mut Stage) -> Vec<UseItemAt> {
        stage.app.world_mut().resource_mut::<Messages<Intent<UseItemAt>>>().drain().map(|i| i.action).collect()
    }

    /// A use aimed somewhere is aimed as an ability is: the cursor opens on
    /// the nearest foe, shows the burst it would land and who stands in
    /// it, and confirming writes the use at the cell.
    #[test]
    fn an_aimed_use_opens_on_the_nearest_foe_previews_its_footprint_and_confirm_uses_it_there() {
        let mut stage = staged();
        let rod = carry(&mut stage, "rod", BURST);
        stage.actor("near", 'n', 2, 0);
        stage.actor("far", 'f', 4, 0);
        stage.tick();
        let (user, at) = (stage.player, stage.at);

        stage.app.world_mut().write_message(AimUse { user, item: rod });
        stage.tick();
        let view = stage.app.world().resource::<TargetView>();
        assert!(view.aiming() && view.using == Some(rod), "the cursor is up on the rod");
        assert_eq!((view.cursor, view.what.as_str()), (at.offset(2, 0), "rod"), "on the nearest foe, by the thing's name");
        assert_eq!(view.span.map(|s| s.max), Some(6), "reaching as far as its aim does");
        let mut cells = view.cells.clone();
        let mut disc: Vec<Point> = rl_core::geometry::disc(at.offset(2, 0), 1).collect();
        cells.sort_by_key(|p| (p.y, p.x));
        disc.sort_by_key(|p| (p.y, p.x));
        assert_eq!(cells, disc, "the burst it will land");
        assert_eq!(view.targets.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(), vec!["near"]);
        assert!(view.legal);

        let _ = aimed_uses(&mut stage);
        stage.press(KeyCode::Enter);
        assert_eq!(aimed_uses(&mut stage), vec![UseItemAt { item: rod, at: at.offset(2, 0) }], "used where it was aimed");
        assert!(!stage.app.world().resource::<TargetView>().aiming(), "and the cursor went away");
    }

    /// A thing whose use lands on its user is used at once and no screen
    /// opens, so a game binds every use the same way and never asks which
    /// kind it is, as it binds abilities.
    #[test]
    fn asked_to_aim_a_use_that_lands_on_its_user_the_cursor_uses_it_at_once() {
        let mut stage = staged();
        let draught = carry(&mut stage, "draught", r#"[(on: "use", effects: [(kind: "Struck")])]"#);
        let user = stage.player;
        stage.app.world_mut().write_message(AimUse { user, item: draught });
        stage.tick();
        let used: Vec<Entity> = stage.app.world_mut().resource_mut::<Messages<Intent<UseItem>>>().drain().map(|i| i.action.0).collect();
        assert_eq!(used, vec![draught], "used where it stands");
        assert!(!stage.app.world().resource::<Modals>().any_open(), "and nothing opened");
    }

    /// A thing that cannot be used now opens no cursor, as something that
    /// cannot be thrown opens none for a throw: an empty one, and one that
    /// is worn to be used and is not, by the rule the resolver refuses a
    /// use by, asked before the aim rather than after it.
    #[test]
    fn asked_to_aim_a_thing_that_cannot_be_used_now_the_cursor_opens_nothing() {
        let mut stage = staged();
        let empty = carry(&mut stage, "empty rod", BURST);
        stage.app.world_mut().entity_mut(empty).insert(Consumable { left: 0, ..Consumable::new(3, WhenEmpty::Kept) });
        let unworn = carry(&mut stage, "unworn rod", BURST);
        stage.app.world_mut().entity_mut(unworn).insert(Wearable(rl_rules::EquipShape::in_slot(rl_rules::SlotId::from_raw(0))));
        let user = stage.player;
        for item in [empty, unworn] {
            stage.app.world_mut().write_message(AimUse { user, item });
            stage.tick();
            assert!(!stage.app.world().resource::<TargetView>().aiming(), "no aim for {item:?}");
            assert!(!stage.app.world().resource::<Modals>().any_open(), "and no screen");
        }
    }

    /// An empty thing not yet known is aimed as any other, since a cursor
    /// that would not open would say it was empty.
    #[test]
    fn asked_to_aim_an_empty_thing_not_yet_known_the_cursor_opens() {
        let mut stage = staged();
        let rod = carry(&mut stage, "teak rod", BURST);
        stage.app.world_mut().entity_mut(rod).insert((Consumable { left: 0, ..Consumable::new(3, WhenEmpty::Kept) }, rl_bevy::Unknown));
        let user = stage.player;
        stage.app.world_mut().write_message(AimUse { user, item: rod });
        stage.tick();
        let view = stage.app.world().resource::<TargetView>();
        assert!(view.aiming() && view.using == Some(rod) && view.legal, "aimed, and taken");
    }

    /// A thing whose use asks for another of its user's things is chosen
    /// for in the bag rather than aimed: the cursor opens no aim for it and
    /// does not use it on nothing.
    #[test]
    fn asked_to_aim_a_use_that_asks_for_a_thing_the_cursor_leaves_it_to_the_bag() {
        let mut stage = staged();
        let polish = carry(&mut stage, "polish", r#"[(on: "use", asks: (prompt: "Polish which?"), effects: [(kind: "Struck")])]"#);
        let user = stage.player;
        stage.app.world_mut().write_message(AimUse { user, item: polish });
        stage.tick();
        assert!(!stage.app.world().resource::<TargetView>().aiming(), "no aim");
        assert!(stage.app.world_mut().resource_mut::<Messages<Intent<UseItem>>>().drain().next().is_none(), "and no use on nothing");
    }

    /// A thing not yet known is aimed with nothing its targeting would
    /// tell: named, with a plain line to the cursor and the cursor's cell,
    /// and no footprint, reach, reason to refuse or anyone named as caught,
    /// even pointed far past where a burst of its reach could go.
    #[test]
    fn an_unknown_things_aim_shows_a_plain_line_and_nothing_its_targeting_would_tell() {
        let mut stage = staged();
        let rod = carry(&mut stage, "teak rod", BURST);
        stage.app.world_mut().entity_mut(rod).insert(rl_bevy::Unknown);
        stage.actor("near", 'n', 2, 0);
        stage.tick();
        let (user, at) = (stage.player, stage.at);
        stage.app.world_mut().write_message(AimUse { user, item: rod });
        stage.tick();
        for cursor in [at.offset(2, 0), at.offset(9, 3)] {
            stage.app.world_mut().resource_mut::<TargetView>().cursor = cursor;
            stage.tick();
            let view = stage.app.world().resource::<TargetView>();
            assert!(view.aiming() && view.unknown, "aimed, and said to be unknown");
            assert_eq!((view.what.as_str(), view.span), ("teak rod", None), "by its name, and no reach");
            let line: Vec<Point> = rl_core::geometry::line(at, cursor).skip(1).collect();
            assert_eq!((&view.path, &view.cells), (&line, &vec![cursor]), "a plain line and the cursor's cell, at {cursor:?}");
            assert!(view.beyond.is_empty() && view.why.is_empty() && view.targets.is_empty(), "nothing out of reach, refused or caught");
            assert!(view.legal, "and taken, as the resolver takes it");
        }
    }

    /// A thing not yet known cycles as one aimed at anyone would, so a
    /// use that wants foes still stops on a friend, rather than telling
    /// what it wants by passing friends by; known, it passes them by.
    #[test]
    fn an_unknown_foe_only_things_cursor_cycles_to_a_friend() {
        let mut stage = staged();
        let (known, unknown) = (
            carry(&mut stage, "rod", r#"[(on: "use", targeting: (mode: Bolt(range: 6)), effects: [(kind: "Struck")])]"#),
            carry(&mut stage, "teak rod", r#"[(on: "use", targeting: (mode: Bolt(range: 6)), effects: [(kind: "Struck")])]"#),
        );
        stage.app.world_mut().entity_mut(unknown).insert(rl_bevy::Unknown);
        let friend = stage.actor("friend", 'f', 2, 0);
        let player = stage.player;
        let ours = stage.app.world().get::<Faction>(player).copied().expect("the player's side");
        stage.app.world_mut().entity_mut(friend).insert(ours);
        stage.tick();
        let beside = stage.at.offset(2, 0);
        // Every cell a few presses of the cycle key visit.
        let mut stops = |item: Entity| {
            stage.app.world_mut().write_message(AimUse { user: player, item });
            stage.tick();
            let mut visited = Vec::new();
            for _ in 0..3 {
                stage.press(KeyCode::Tab);
                visited.push(stage.app.world().resource::<TargetView>().cursor);
            }
            stage.press(KeyCode::Escape);
            visited
        };
        assert!(!stops(known).contains(&beside), "known, a foe-only rod passes a friend by");
        assert!(stops(unknown).contains(&beside), "not yet known, it stops on one");
    }

    /// The property the preview exists for, held for a thing's aimed use as
    /// it is for an ability, over many layouts: whether the box reads as
    /// refused, and who it lists, are exactly whether the resolver takes
    /// the use and who it lands on once the player confirms.
    #[test]
    fn the_preview_of_an_aimed_use_names_exactly_who_it_lands_on() {
        use rl_core::seed::position_hash;
        // How many aims of each kind the span met, so a property that held
        // only because every aim was refused, or landed on nobody, fails.
        let (mut took, mut refused, mut struck_someone) = (0, 0, 0);
        for seed in 0..12u64 {
            // A small deterministic stream: one hash per draw.
            let mut draw = 0;
            let mut roll = |n: i32| {
                draw += 1;
                (position_hash(seed, draw, n) % n as u64) as i32
            };
            let mut stage = staged();
            let rods = [
                carry(&mut stage, "bolt", r#"[(on: "use", targeting: (mode: Bolt(range: 6)), effects: [(kind: "Struck")])]"#),
                carry(&mut stage, "burst", BURST),
                carry(&mut stage, "salve", r#"[(on: "use", targeting: (mode: Ball(range: 6, radius: 1), aim: Ally), effects: [(kind: "Struck")])]"#),
            ];
            let player = stage.player;
            if roll(2) == 0 {
                stage.app.world_mut().get_mut::<Health>(player).expect("health").current = 12;
            }
            let mut taken = vec![(0, 0)];
            for i in 0..6 {
                let (dx, dy) = loop {
                    let at = (roll(9) - 4, roll(9) - 4);
                    if !taken.contains(&at) {
                        break at;
                    }
                };
                taken.push((dx, dy));
                let who = stage.actor(&format!("them {i}"), 't', dx, dy);
                if roll(2) == 0 {
                    stage.app.world_mut().entity_mut(who).insert(Faction(rl_rules::FactionId::from_raw(0)));
                }
                if roll(2) == 0 {
                    stage.app.world_mut().get_mut::<Health>(who).expect("health").current = 4;
                }
            }
            stage.tick();
            stage.tick();

            for _ in 0..6 {
                let item = rods[roll(3) as usize];
                let aim = stage.at.offset(roll(13) - 6, roll(13) - 6);
                stage.app.world_mut().write_message(AimUse { user: player, item });
                stage.tick();
                stage.app.world_mut().resource_mut::<TargetView>().cursor = aim;
                stage.tick();
                let view = stage.app.world().resource::<TargetView>();
                assert!(view.aiming(), "seed {seed}: the cursor is up");
                let (legal, why) = (view.legal, view.why.clone());
                let mut shown: Vec<Entity> = view.targets.iter().map(|r| r.entity).collect();
                shown.sort();

                let world = stage.app.world_mut();
                let marked: Vec<Entity> = world.query_filtered::<Entity, With<Hit>>().iter(world).collect();
                for e in marked {
                    world.entity_mut(e).remove::<Hit>();
                }
                let _ = world.resource_mut::<Messages<ItemEvent>>().drain().count();
                stage.press(KeyCode::Enter);
                let world = stage.app.world_mut();
                let used = world.resource_mut::<Messages<ItemEvent>>().drain().any(|e| matches!(e, ItemEvent::Used { item: used, .. } if used == item));
                let mut hit: Vec<Entity> = world.query_filtered::<Entity, With<Hit>>().iter(world).collect();
                hit.sort();
                if legal {
                    assert!(used, "seed {seed}: the preview called legal an aim the resolver refused");
                    assert_eq!(hit, shown, "seed {seed}: the box listed other than who it landed on");
                    took += 1;
                    struck_someone += usize::from(!hit.is_empty());
                } else {
                    assert!(!used, "seed {seed}: the resolver took an aim the preview refused ({why:?})");
                    assert!(hit.is_empty(), "seed {seed}: a refused use landed on {hit:?}");
                    refused += 1;
                }
            }
        }
        assert!(took > 0 && refused > 0 && struck_someone > 0, "the span met {took} taken, {refused} refused, {struck_someone} landing on someone");
    }
}
