//! The player's standard keys, turned into the engine's intents.
//!
//! Walking, taking the way underfoot, waiting, picking up, firing and
//! throwing are the same six branches of input in every roguelike the
//! engine has carried: a direction key is a [`Bump`], whatever the turn
//! loop then makes of it; fire opens the targeting cursor with whatever
//! shoots; throw opens the pack on the first thing that can be thrown.
//! Every game wrote them for itself, line for line. [`PlayerKeysPlugin`]
//! is that system written once.
//!
//! A game names the verbs it wants and the words the controls screen lists
//! each under, and leaves the others out. Its own verbs stay in its own
//! input system, ordered against [`PlayerKeysSet`] rather than against the
//! function here, so a key of its own that opens a screen can run first
//! and a key that reads what the player asked for can run after.
//!
//! ```
//! # use bevy::prelude::*;
//! # use atglyph_ui::{Otherwise, PlayerKeysPlugin, PlayerKeysSet, UiPlugin};
//! # fn my_own_verbs() {}
//! # let mut app = App::new();
//! app.add_plugins(UiPlugin).add_plugins(
//!     PlayerKeysPlugin::new()
//!         .walk("Move", "walk, or strike whoever is there")
//!         .wait("Act", "wait a turn")
//!         .fire("Act", "shoot", Otherwise::say("You have nothing to shoot with.")),
//! );
//! app.add_systems(Update, my_own_verbs.after(PlayerKeysSet));
//! ```
//!
//! Nothing here is a rule. Each verb writes the engine's own intent and
//! the turn loop decides what it comes to. A key with nothing to act on,
//! fire with nothing that shoots or throw with nothing to throw, spends no
//! turn, since a mistyped key should not cost a life: it says the game's
//! line if the game gave one, and writes a [`Declined`] either way, for a
//! game that would rather say why in its own words from its own state.
//!
//! A plugin rather than a function because a game turns it on or off whole:
//! one with another scheme, or the tutorial's early steps, which teach the
//! hand-written loop, leave it out, and then nothing here runs.

use atglyph_bevy::EngineSet;
use atglyph_bevy::prelude::*;
use bevy::prelude::*;

use crate::controls::{AddControls, Chord, ControlId, ControlInput, Keys};
use crate::log::Log;
use crate::modal::Modals;
use crate::panel::{InventoryMenu, inventory_modal};
use crate::tone::{ToneId, Tones};
use crate::view::{AimFire, InventoryView, ItemRow};

/// One of the standard verbs a key asks for.
///
/// Closed, because it enumerates what [`PlayerKeysPlugin`] reads; a game's
/// own verbs are its own controls and never need a variant here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PlayerVerb {
    /// A [`Bump`] in a direction: a step, a blow, or a door opened.
    Walk,
    /// A [`GoThrough`] the way underfoot.
    GoThrough,
    /// A [`Wait`].
    Wait,
    /// A [`PickUp`] of what lies underfoot.
    PickUp,
    /// An [`AimFire`], which opens the targeting cursor, when the player
    /// has a ranged attack.
    Fire,
    /// The pack opened on the first thing in it that can be thrown, whose
    /// own throw key sends it to the targeting cursor.
    Throw,
}

impl PlayerVerb {
    /// The keys the verb answers to unless the game binds others.
    ///
    /// The engine's own bindings where there are any: the walk is every
    /// direction key [`DirectionKeys`](crate::DirectionKeys) binds, and the
    /// throw is the pack's own throw key. The rest are the keys every
    /// roguelike the engine has carried chose: Enter, `>` and `<` for the
    /// way underfoot, `.` and the numpad's middle for a wait, `g` and `,`
    /// to pick up, `f` to fire.
    pub(crate) fn default_keys(self) -> Keys {
        use KeyCode::*;
        match self {
            PlayerVerb::Walk => Keys::Directions { shift: false },
            PlayerVerb::GoThrough => [Chord::key(Enter), Chord::shift(Period), Chord::shift(Comma)].into(),
            PlayerVerb::Wait => [Period, Numpad5].into(),
            PlayerVerb::PickUp => [KeyG, Comma].into(),
            PlayerVerb::Fire => KeyF.into(),
            PlayerVerb::Throw => crate::InventoryKeys::default().throw.into(),
        }
    }
}

/// What a key with nothing to act on says in the log.
///
/// The words are always the game's: the engine says nothing of its own.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Otherwise {
    /// Nothing in the log; the [`Declined`] is still written.
    #[default]
    Nothing,
    /// A line in a tone.
    Say {
        /// The line.
        text: String,
        /// Its tone.
        tone: ToneId,
    },
}

impl Otherwise {
    /// The line, in [`Tones::MUTED`], as something that matters less than
    /// what the turns did.
    pub fn say(text: impl Into<String>) -> Self {
        Otherwise::Say { text: text.into(), tone: Tones::MUTED }
    }
}

/// A standard key the player pressed that had nothing to act on: fire with
/// nothing that shoots, or throw with nothing that can be thrown. No turn
/// was spent.
///
/// Written whatever [`Otherwise`] the verb says, for a game that explains
/// why from its own state: which gun is too hot, which is dry. Read it in
/// a system after [`PlayerKeysSet`].
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Declined {
    /// The player who pressed it.
    pub actor: Entity,
    /// What was asked for.
    pub verb: PlayerVerb,
}

/// Where the standard keys are read, inside
/// [`EngineSet::Input`].
///
/// A game's own keys order themselves against this set: before it, a key
/// that opens a screen, since the standard keys stand aside while a screen
/// is up; after it, a system that reads a [`Declined`].
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlayerKeysSet;

/// One verb as the game asked for it.
#[derive(Debug, Clone)]
struct Wanted {
    verb: PlayerVerb,
    group: String,
    action: String,
    keys: Keys,
    otherwise: Otherwise,
}

/// The standard keys, for the verbs a game names.
///
/// Each verb is declared in the controls registry under the group and the
/// action the game gives, in the order the game names them, so the
/// controls screen lists them where the game put them. A verb left out is
/// neither declared nor read.
#[derive(Debug, Clone, Default)]
pub struct PlayerKeysPlugin {
    wanted: Vec<Wanted>,
    thrown_first: Option<String>,
}

impl PlayerKeysPlugin {
    /// No verbs yet.
    pub fn new() -> Self {
        Self::default()
    }

    fn with(mut self, verb: PlayerVerb, group: &str, action: &str, otherwise: Otherwise) -> Self {
        assert!(self.wanted.iter().all(|w| w.verb != verb), "PlayerKeysPlugin: {verb:?} is named twice");
        self.wanted.push(Wanted { verb, group: group.to_string(), action: action.to_string(), keys: verb.default_keys(), otherwise });
        self
    }

    /// Walking, listed as `action` under `group`.
    pub fn walk(self, group: &str, action: &str) -> Self {
        self.with(PlayerVerb::Walk, group, action, Otherwise::Nothing)
    }

    /// Taking the way underfoot, listed as `action` under `group`.
    pub fn go_through(self, group: &str, action: &str) -> Self {
        self.with(PlayerVerb::GoThrough, group, action, Otherwise::Nothing)
    }

    /// Waiting a turn, listed as `action` under `group`.
    pub fn wait(self, group: &str, action: &str) -> Self {
        self.with(PlayerVerb::Wait, group, action, Otherwise::Nothing)
    }

    /// Picking up what is underfoot, listed as `action` under `group`.
    pub fn pick_up(self, group: &str, action: &str) -> Self {
        self.with(PlayerVerb::PickUp, group, action, Otherwise::Nothing)
    }

    /// Firing through the targeting cursor, listed as `action` under
    /// `group`, saying `otherwise` when the player has no ranged attack.
    pub fn fire(self, group: &str, action: &str, otherwise: Otherwise) -> Self {
        self.with(PlayerVerb::Fire, group, action, otherwise)
    }

    /// Opening the pack on the first thing that can be thrown, listed as
    /// `action` under `group`, saying `otherwise` when nothing can be.
    ///
    /// Does nothing at all in a game with no pack screen, which has
    /// nothing to open: neither the line nor a [`Declined`].
    pub fn throw(self, group: &str, action: &str, otherwise: Otherwise) -> Self {
        self.with(PlayerVerb::Throw, group, action, otherwise)
    }

    /// Binds `verb` to `keys` in place of its default.
    ///
    /// # Panics
    /// Panics when `verb` was not named first: a key for a verb nobody
    /// reads is a mistake.
    pub fn keys(mut self, verb: PlayerVerb, keys: impl Into<Keys>) -> Self {
        let wanted = self.wanted.iter_mut().find(|w| w.verb == verb);
        wanted.unwrap_or_else(|| panic!("PlayerKeysPlugin: a key for {verb:?}, which was not named")).keys = keys.into();
        self
    }

    /// Opens the throw on the first thing tagged `tag` before anything else
    /// that can be thrown: what is made to be thrown before what merely can
    /// be. A tag the game never registered prefers nothing.
    pub fn throw_first(mut self, tag: impl Into<String>) -> Self {
        self.thrown_first = Some(tag.into());
        self
    }
}

impl Plugin for PlayerKeysPlugin {
    fn build(&self, app: &mut App) {
        let mut verbs: Vec<(PlayerVerb, ControlId, Otherwise)> =
            self.wanted.iter().map(|w| (w.verb, app.add_control(&w.group, &w.action, w.keys.clone()), w.otherwise.clone())).collect();
        // Read in one fixed order whatever order they were listed in, so a
        // press two verbs share asks for the one it always did.
        verbs.sort_by_key(|(verb, ..)| *verb);
        app.insert_resource(PlayerKeys { verbs, thrown_first: self.thrown_first.clone() })
            .add_message::<Declined>()
            // A no-op beside the targeting cursor's own plugin, and what lets
            // a test with no cursor press the key all the same.
            .add_message::<AimFire>()
            .configure_sets(Update, PlayerKeysSet.in_set(EngineSet::Input))
            .add_systems(Update, player_keys.in_set(PlayerKeysSet));
    }

    fn finish(&self, app: &mut App) {
        atglyph_bevy::depends_on::<crate::UiPlugin>(app, "PlayerKeysPlugin");
        if self.wanted.iter().any(|w| w.verb == PlayerVerb::PickUp) {
            atglyph_bevy::depends_on::<ItemsPlugin>(app, "PlayerKeysPlugin's pick_up");
        }
    }
}

/// The verbs [`PlayerKeysPlugin`] declared, and what each says with
/// nothing to act on.
#[derive(Resource, Debug, Clone)]
pub(crate) struct PlayerKeys {
    verbs: Vec<(PlayerVerb, ControlId, Otherwise)>,
    thrown_first: Option<String>,
}

impl PlayerKeys {}

/// The player while it holds the turn.
type PlayerTurn<'w, 's> = Query<'w, 's, Entity, (With<Player>, With<MyTurn>)>;

/// What every key here reads before it asks for anything.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Keyboard<'w, 's> {
    keys: ControlInput<'w>,
    wanted: Res<'w, PlayerKeys>,
    modals: ResMut<'w, Modals>,
    log: Log<'w>,
    player: PlayerTurn<'w, 's>,
}

/// What the keys can ask for, and the word that they could not.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Asks<'w> {
    bumps: MessageWriter<'w, Intent<Bump>>,
    waits: MessageWriter<'w, Intent<Wait>>,
    transits: MessageWriter<'w, Intent<GoThrough>>,
    /// Absent from a game with no items, whose keys never pick up; one
    /// that names the pick-up is held to adding `ItemsPlugin`.
    pick_ups: Option<MessageWriter<'w, Intent<PickUp>>>,
    fires: MessageWriter<'w, AimFire>,
    declined: MessageWriter<'w, Declined>,
}

/// The pack's screen, which the throw opens: absent from a game or a test
/// that draws no screens, where the throw has nothing to open.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Pack<'w, 's> {
    view: Option<Res<'w, InventoryView>>,
    menu: Option<ResMut<'w, InventoryMenu>>,
    registries: Option<Res<'w, Registries>>,
    tagged: Query<'w, 's, &'static Tagged>,
}

/// Turns the standard keys into an intent for the player while it holds
/// the turn, and nothing at all while a screen is up.
///
/// At most one verb a press, checked walk first and throw last.
pub(crate) fn player_keys(mut kb: Keyboard, loadout: Loadout, mut pack: Pack, mut asks: Asks) {
    if kb.modals.any_open() {
        return;
    }
    let Ok(me) = kb.player.single() else { return };
    let Keyboard { keys, wanted, modals, log, .. } = &mut kb;
    let pressed = wanted.verbs.iter().find_map(|(verb, id, otherwise)| match verb {
        PlayerVerb::Walk => keys.direction(*id).map(|dir| (*verb, Some(dir), otherwise)),
        _ => keys.just_pressed(*id).then_some((*verb, None, otherwise)),
    });
    let Some((verb, dir, otherwise)) = pressed else { return };
    let declined = match verb {
        PlayerVerb::Walk => {
            asks.bumps.write(Intent::new(me, Bump(dir.expect("a walk has a direction"))));
            false
        }
        PlayerVerb::GoThrough => {
            asks.transits.write(Intent::new(me, GoThrough));
            false
        }
        PlayerVerb::Wait => {
            asks.waits.write(Intent::new(me, Wait));
            false
        }
        PlayerVerb::PickUp => {
            if let Some(pick_ups) = asks.pick_ups.as_mut() {
                pick_ups.write(Intent::new(me, PickUp));
            }
            false
        }
        PlayerVerb::Fire => {
            let ready = loadout.ranged(me).is_some();
            if ready {
                asks.fires.write(AimFire { user: me });
            }
            !ready
        }
        PlayerVerb::Throw => {
            let (Some(view), Some(menu)) = (pack.view.as_deref(), pack.menu.as_deref_mut()) else { return };
            // What the game says is made to be thrown comes first: a potion
            // can be thrown, but a hand reaching for the key means the darts
            // before the healing.
            let tag = wanted.thrown_first.as_deref().and_then(|name| pack.registries.as_deref().and_then(|r| r.tags.id(name)));
            let made_for_it = |row: &&ItemRow| tag.is_some_and(|tag| pack.tagged.get(row.entity).is_ok_and(|tags| tags.contains(&tag)));
            let first = view.rows.iter().position(|row| row.throw_range.is_some() && made_for_it(&row));
            match first.or_else(|| view.rows.iter().position(|row| row.throw_range.is_some())) {
                Some(row) => {
                    menu.open(row);
                    let bag = inventory_modal(modals);
                    modals.open(bag);
                    false
                }
                None => true,
            }
        }
    };
    if declined {
        asks.declined.write(Declined { actor: me, verb });
        if let Otherwise::Say { text, tone } = otherwise {
            log.push(text.clone(), *tone);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;
    use crate::log::MessageLog;

    fn said(stage: &Stage) -> Vec<String> {
        stage.app.world().resource::<MessageLog>().iter().map(|line| line.text.clone()).collect()
    }

    fn standard() -> PlayerKeysPlugin {
        PlayerKeysPlugin::new()
            .walk("Move", "walk")
            .go_through("Move", "go through")
            .wait("Act", "wait")
            .pick_up("Act", "pick up")
            .fire("Act", "fire", Otherwise::say("Nothing to fire."))
            .throw("Act", "throw", Otherwise::Nothing)
    }

    /// The smallest game there is: the engine's core, sight and the UI base,
    /// and only walking and waiting. No items, so nothing ever registered
    /// a pick-up for the keys to ask for.
    #[test]
    fn a_game_with_nothing_to_pick_up_walks_on_the_standard_keys() {
        let mut app = atglyph_bevy::testing::headless_app();
        app.add_plugins((FovPlugin, atglyph_bevy::testing::KeyScriptPlugin, crate::UiPlugin))
            .add_plugins(PlayerKeysPlugin::new().walk("Move", "walk").wait("Act", "wait"));
        let at = atglyph_bevy::testing::ground(&mut app);
        let me = app.world_mut().spawn((Actor, Player, Blocks, Position(at), Viewshed::new(6), RevealsMap)).id();
        app.finish();
        app.cleanup();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        atglyph_bevy::testing::settle(&mut app);
        atglyph_bevy::testing::press(&mut app, KeyCode::KeyL);
        app.update();
        assert_eq!(app.world().get::<Position>(me).unwrap().0, at.offset(1, 0), "a step east");
    }

    #[test]
    fn a_direction_key_bumps_the_player_that_way() {
        let mut stage = Stage::new(standard());
        let before = stage.at;
        stage.press(KeyCode::KeyL);
        stage.tick();
        assert_eq!(stage.app.world().get::<Position>(stage.player).unwrap().0, before.offset(1, 0), "a step east");
    }

    #[test]
    fn fire_with_nothing_to_shoot_says_the_games_line_declines_and_spends_no_turn() {
        let mut stage = Stage::new(standard());
        stage.app.init_resource::<Messages<Declined>>();
        let before = stage.app.world().resource::<Turns>().now();
        stage.press(KeyCode::KeyF);
        assert_eq!(said(&stage), vec!["Nothing to fire.".to_string()]);
        assert_eq!(stage.app.world().resource::<Turns>().now(), before, "no turn spent");
        let declined: Vec<Declined> = stage.app.world_mut().resource_mut::<Messages<Declined>>().drain().collect();
        assert_eq!(declined, vec![Declined { actor: stage.player, verb: PlayerVerb::Fire }]);
    }

    #[test]
    fn fire_with_a_ranged_attack_opens_the_aim() {
        let mut stage = Stage::new(standard());
        let kind = stage.kind;
        stage.app.world_mut().entity_mut(stage.player).insert(RangedAttack::new(kind, atglyph_core::DiceRoll::new(1, 4), 6));
        stage.press(KeyCode::KeyF);
        let fires: Vec<AimFire> = stage.app.world_mut().resource_mut::<Messages<AimFire>>().drain().collect();
        assert_eq!(fires.len(), 1, "one aim asked for");
        assert!(said(&stage).is_empty(), "and nothing said");
    }

    #[test]
    fn a_verb_the_game_left_out_is_neither_listed_nor_read() {
        let mut stage = Stage::new(PlayerKeysPlugin::new().walk("Move", "walk"));
        let before = stage.app.world().resource::<Turns>().now();
        stage.press(KeyCode::Period);
        stage.tick();
        assert_eq!(stage.app.world().resource::<Turns>().now(), before, "no wait was asked for");
        let controls = stage.app.world().resource::<crate::Controls>();
        assert!(controls.find("Act", "wait").is_none());
        assert!(controls.find("Move", "walk").is_some());
    }

    #[test]
    fn a_rebound_verb_answers_to_its_new_key_only() {
        let mut stage = Stage::new(PlayerKeysPlugin::new().fire("Act", "fire", Otherwise::say("No.")).keys(PlayerVerb::Fire, KeyCode::KeyZ));
        stage.press(KeyCode::KeyF);
        assert!(said(&stage).is_empty(), "f is no longer fire");
        stage.press(KeyCode::KeyZ);
        assert_eq!(said(&stage), vec!["No.".to_string()]);
    }

    #[test]
    fn nothing_is_read_while_a_screen_is_up() {
        let mut stage = Stage::new(standard());
        let modal = stage.app.world_mut().resource_mut::<Modals>().declare("a screen");
        stage.app.world_mut().resource_mut::<Modals>().open(modal);
        stage.press(KeyCode::KeyF);
        assert!(said(&stage).is_empty());
    }

    #[test]
    #[should_panic(expected = "which was not named")]
    fn a_key_for_a_verb_not_named_is_refused() {
        let _ = PlayerKeysPlugin::new().keys(PlayerVerb::Wait, KeyCode::KeyZ);
    }
}
