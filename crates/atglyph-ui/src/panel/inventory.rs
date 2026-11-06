//! The bag: what you carry, what each thing is worth, and what to do with
//! it.
//!
//! A modal the engine runs end to end, the way the ability menu is:
//! [`InventoryKeys::toggle`] opens it, and it works in two steps so that no
//! key means two things on one screen. On the list, each row is lettered
//! `a` to `z` in the order it is listed, and its letter picks it out and
//! opens it; the direction keys walk the rows, held as well as pressed, and
//! the cursors' confirm key opens the row under the cursor, which is how a
//! row past `z` is reached. An opened row takes the bag's action keys, and
//! the close key goes back to the list. So `d` on the list is the fourth
//! row and `d` on an opened row drops it, and neither is ever a guess.
//!
//! Wearing, dropping and using are the engine's own item actions, so the
//! screen writes their intents itself; throwing opens the targeting cursor
//! through [`AimThrow`], the way a game's throw key would. A game binds
//! nothing. Using an item with a `use` trigger uses it where it stands, and
//! using one whose use is aimed opens the targeting cursor through
//! `AimUse`, as throwing does, and puts it away again for nothing. Using
//! one whose use asks for another of the player's things turns the bag to
//! choosing, titled with the use's own prompt and listing only what it may
//! be made on, lettered in its own order: a letter or confirm writes
//! [`UseOn`] for the thing picked out, and putting the bag away spends
//! nothing; a game's own use key writing `AimUse` for such a thing opens
//! the bag choosing the same way. An offer that chooses first,
//! [`Offering::choosing`], taken up on the offers screen or by the interact
//! key, opens the bag choosing the same way, under the offer's prompt among
//! what the game's system names: a letter or confirm takes the offer up
//! with the thing picked out, the [`Interact`] the engine checks and
//! charges, and putting the bag away takes nothing up and costs nothing.
//! Nothing else is used from here; an item a game answers itself from
//! [`ItemEvent::Used`] is used by the game's own key, since the bag cannot
//! know it does anything and a use that did nothing would spend a turn.
//! The footer offers only the keys that do something: on the list, the
//! letters and the key that opens; on an opened row, wear or take off,
//! drop, use, throw, and back, and for a worn thing that will not come off,
//! none that would take it off.
//!
//! Under the rows, the row picked out is described from its own
//! components: the blow it is swung with, the shot it fires, what it adds
//! to armor or a stat, what it resists, how far it flies, where it is worn
//! or could be and what else it takes, whether it will come off, what it
//! does at each of its moments and how a use is aimed, how many charges
//! are left, and whatever the game pushed onto it in
//! [`ViewSet::Annotate`](crate::ViewSet). A thing not yet known is
//! described as unknown, by where it goes and what the game pushed, and by
//! nothing else; what the game says it seems, in
//! [`ItemRow::seems`](crate::view::inventory::ItemRow::seems), is said in
//! place of the bare word.
//!
//! A thing that could be worn and is not is then weighed against what it
//! would take off, by its
//! [`replacing`](crate::view::inventory::ItemRow::replacing): what it
//! replaces in the slot it goes in, or that nothing is worn there, the
//! hand a two-hander takes from what holds it and the hand a one-hander
//! frees, and each number that would change, old to new and by how much,
//! a gain in the good tone and a loss in the bad. Picking a row out puts
//! nothing on.
//!
//! Every action closes every screen, since it spends a turn and the turn
//! loop assumes nothing is up while it runs.

use crate::app::UiApp;
use atglyph_bevy::prelude::*;
use atglyph_bevy::{EngineSet, PresentSet};
use atglyph_core::{Direction, GridRect};
use atglyph_render::{Cell, Terminal};
use bevy::prelude::*;

use crate::controls::{Chord, ControlInput, EngineKey, is_letter, key_name, lettered_row, row_letter};
use crate::modal::{ModalId, Modals};
use crate::panel::ability::reach;
use crate::panel::sheet::plain_op;
use crate::panel::{clear, clip, frame, wrap};
use crate::tone::{Palette, ToneId, Tones};
use crate::view::ability::turns;
use crate::view::inventory::{Bestowed, InventoryView, InventoryViewPlugin, ItemRow, Kit, LaunchLine, Replacing, TakenOff, Wielding};
use crate::view::sheet::{ResistLine, Strike};
use crate::view::target::{AimThrow, AimUse};
use atglyph_rules::stats::Op;

/// The name the bag's modal is declared under.
pub(crate) const INVENTORY_MODAL: &str = "inventory";

/// The keys the bag answers to. Walking the rows and closing use the
/// direction and cursor keys every other screen uses, and the cursors'
/// confirm key opens the row under the cursor and uses an opened one.
///
/// Every key but the toggle is read on an opened row only, since on the
/// list each letter is a row's.
#[derive(Resource, Debug, Clone)]
pub struct InventoryKeys {
    /// Opens and closes the bag. A plain letter is a row's on the list, so
    /// a toggle that is one closes the bag from an opened row, and the
    /// close key closes it from the list.
    pub toggle: Chord,
    /// Puts the opened row on, or takes it off.
    pub wear: Chord,
    /// Drops it.
    pub drop: Chord,
    /// Uses it, as does the cursors' confirm key.
    pub use_it: Chord,
    /// Throws it, through the targeting cursor.
    pub throw: Chord,
}

impl Default for InventoryKeys {
    fn default() -> Self {
        Self {
            toggle: Chord::key(KeyCode::KeyI),
            wear: Chord::key(KeyCode::KeyE),
            drop: Chord::key(KeyCode::KeyD),
            use_it: Chord::key(KeyCode::KeyU),
            throw: Chord::key(KeyCode::KeyT),
        }
    }
}

/// Where the bag is drawn, and what it is called.
#[derive(Resource, Debug, Clone)]
pub(crate) struct InventoryLayout {
    /// The terminal cells it occupies.
    pub rect: GridRect,
    /// Drawn in the top border. The engine has no word for a sea chest.
    pub title: String,
    /// What the bag is called where the engine names it, on the controls
    /// screen.
    pub what: String,
    /// Shown in place of rows when there are none.
    pub empty: String,
    /// What the close key is said to do while the bag is choosing, a thing
    /// for a use or for an offer: backing out, which takes nothing up, so
    /// the footer says so rather than naming a bare key.
    pub never_mind: String,
    /// What the wearer's own blow and shot are called where gear is
    /// weighed.
    pub own: String,
}

/// Which row the cursor is on, whether it is opened, and what the bag is
/// choosing for. The screen's own state, since a view holds no cursor.
#[derive(Resource, Debug, Default)]
pub struct InventoryMenu {
    /// The row picked out, clamped to the rows there are when drawn.
    pub selected: usize,
    /// Whether that row is opened, taking the action keys, rather than
    /// picked out on the list. Back to the list whenever the bag is put
    /// away, so it always comes up on the list unless a game opens it on a
    /// row with [`open`](Self::open).
    pub opened: bool,
    /// While the bag is choosing, what for: a use, or an offer, one at a
    /// time. Put away whenever the bag is, so it never opens choosing for
    /// something nobody asked for again.
    pub choosing: Option<Choosing>,
}

/// What the bag is choosing for: one thing at a time, so one field rather
/// than two that could both be set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choosing {
    /// What a use is made on: the thing used, and which of its choices
    /// the cursor is on.
    Use(ChoosingFor),
    /// What an offer that chooses first is taken up with.
    Offer(OfferChoice),
}

impl InventoryMenu {
    /// Picks out `row` and opens it, for a game that puts the bag up on
    /// one thing: a throw key that opens it on what can be thrown.
    pub fn open(&mut self, row: usize) {
        self.selected = row;
        self.opened = true;
    }

    /// Moves the cursor `by` rows over `rows` of them, wrapping at either
    /// end.
    pub(crate) fn move_by(&mut self, by: i32, rows: usize) {
        self.selected = stepped(self.selected, by, rows);
    }

    /// The thing whose use the bag is choosing for, while it chooses for
    /// one, and not for an offer.
    pub fn using(&self) -> Option<Entity> {
        match &self.choosing {
            Some(Choosing::Use(choosing)) => Some(choosing.item),
            _ => None,
        }
    }
}

/// The bag choosing what a use is made on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChoosingFor {
    /// The thing whose use asks for another.
    pub item: Entity,
    /// Which of its choices the cursor is on, clamped to the choices there
    /// are when drawn.
    pub selected: usize,
}

/// The bag choosing what an offer that chooses first,
/// [`Offering::choosing`], is taken up with: the offer, its prompt, and
/// what the game's system named when it was taken up.
///
/// The choices are asked of the game once, as the bag opens, rather than
/// every frame, since they are the player's things and nothing moves them
/// while the bag is up; what the bag no longer holds is left out as it is
/// drawn, and the engine refuses a pick no longer carried.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfferChoice {
    /// Who or what offers it.
    pub target: Entity,
    /// Which of its offers.
    pub verb: VerbId,
    /// What the bag is titled while it chooses.
    pub prompt: String,
    /// What it may choose among, in the order listed.
    pub choices: Vec<Entity>,
    /// Which of its choices the cursor is on, clamped to the choices there
    /// are when drawn.
    pub selected: usize,
}

/// What the bag is choosing among, with no screen of its own: a use's
/// choices or a game's question's, each only while the bag holds it.
fn choices_of(menu: &InventoryMenu, view: &InventoryView) -> Option<(String, Vec<Entity>, usize)> {
    let held = |things: &[Entity]| -> Vec<Entity> { things.iter().copied().filter(|t| view.rows.iter().any(|r| r.entity == *t)).collect() };
    match menu.choosing.as_ref()? {
        Choosing::Offer(offer) => Some((offer.prompt.clone(), held(&offer.choices), offer.selected)),
        Choosing::Use(choosing) => {
            let asks = view.rows.iter().find(|r| r.entity == choosing.item)?.asks.as_ref()?;
            Some((asks.prompt.clone(), held(&asks.choices), choosing.selected))
        }
    }
}

/// Moves the cursor of whichever choosing the bag holds to `at`.
fn choose_at(menu: &mut InventoryMenu, at: usize) {
    match menu.choosing.as_mut() {
        Some(Choosing::Offer(offer)) => offer.selected = at,
        Some(Choosing::Use(choosing)) => choosing.selected = at,
        None => {}
    }
}

/// Turns the bag to choosing for `what`, over the whole list rather than
/// an opened row, since what it chooses among is a list of its own.
fn start_choosing(menu: &mut InventoryMenu, what: Choosing) {
    menu.choosing = Some(what);
    menu.opened = false;
}

/// Puts away whatever the bag was choosing for, and the row it had open:
/// how the bag comes up next time, on the list.
fn back_to_the_list(menu: &mut InventoryMenu) {
    menu.choosing = None;
    menu.opened = false;
}

/// `at` moved `by` rows over `rows` of them, wrapping at either end.
pub(crate) fn stepped(at: usize, by: i32, rows: usize) -> usize {
    if rows == 0 {
        return 0;
    }
    (at as i64 + by as i64).rem_euclid(rows as i64) as usize
}

/// Draws [`InventoryView`] as a list with the row picked out described
/// under it, and runs the screen.
///
/// Adds [`InventoryViewPlugin`] if the game has not, and declares the
/// `inventory` modal.
pub struct InventoryPanel(InventoryLayout);

impl InventoryPanel {
    /// The bag in `rect`.
    pub fn new(rect: GridRect) -> Self {
        Self(InventoryLayout {
            rect,
            title: "Inventory".into(),
            what: "inventory".into(),
            empty: "Nothing.".into(),
            never_mind: "never mind".into(),
            own: "own".into(),
        })
    }

    /// Sets what the top border says.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.0.title = title.into();
        self
    }

    /// Sets what the bag is called where the engine names it: on the
    /// controls screen, "open your inventory" becomes "open your `what`".
    pub fn called(mut self, what: impl Into<String>) -> Self {
        self.0.what = what.into();
        self
    }

    /// Sets what an empty bag says.
    pub fn empty(mut self, text: impl Into<String>) -> Self {
        self.0.empty = text.into();
        self
    }

    /// Sets what the wearer's own blow and shot are called where gear is
    /// weighed, `own` by default: a game whose player fights bare-handed
    /// says `fists`, so a dagger put away reads `blow dagger 1d4 -> fists
    /// 1d2`.
    pub fn own(mut self, words: impl Into<String>) -> Self {
        self.0.own = words.into();
        self
    }

    /// Sets what the close key is said to do while the bag is choosing,
    /// `never mind` by default, the offers screen's words.
    pub fn never_mind(mut self, words: impl Into<String>) -> Self {
        self.0.never_mind = words.into();
        self
    }
}

impl Plugin for InventoryPanel {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<InventoryViewPlugin>() {
            app.add_plugins(InventoryViewPlugin);
        }
        app.add_modal(INVENTORY_MODAL);
        // The intents the screen writes, registered whether or not the game
        // added the plugin that resolves each: a game without throwing still
        // has a bag.
        app.add_message::<Intent<Equip>>()
            .add_message::<Intent<Unequip>>()
            .add_message::<Intent<DropItem>>()
            .add_message::<Intent<UseItem>>()
            .add_message::<Intent<UseOn>>()
            .add_message::<AimUse>()
            .add_message::<AimThrow>()
            .add_message::<Intent<Interact>>();
        app.insert_resource(self.0.clone()).init_resource::<InventoryMenu>().init_resource::<InventoryKeys>();
        // A game's own use key opens the bag choosing before the bag reads
        // its keys, so the press that asked is not read again inside it.
        crate::panel::ground::add_count_prompt(app);
        app.add_systems(Update, (choose_when_asked, inventory_keys).chain().in_set(EngineSet::Input).before(crate::panel::ground::count_keys))
            .add_systems(Update, draw_inventory.in_set(PresentSet::Overlay).before(crate::panel::ground::draw_count));
    }

    fn finish(&self, app: &mut App) {
        app.depends_on::<crate::UiPlugin>("InventoryPanel");
        // After the game's own, so its groups are listed first.
        // Only the key that opens it. What the bag's own keys do is
        // written along its bottom border, where a player is already
        // looking; a second list of them on the controls screen is a second
        // place to keep in step, and says nothing the screen does not.
        app.add_control(crate::focus::SCREENS_GROUP, &format!("open your {}", self.0.what), EngineKey::OpenInventory);
    }
}

/// The id of the bag's modal, for a game gating its own systems.
///
/// # Panics
/// Panics if [`InventoryPanel`] was not added.
pub fn inventory_modal(modals: &Modals) -> ModalId {
    modals.get(INVENTORY_MODAL).expect("InventoryPanel declares the inventory modal")
}

/// What the bag can ask for.
#[derive(bevy::ecs::system::SystemParam)]
pub struct BagIntents<'w> {
    equips: MessageWriter<'w, Intent<Equip>>,
    unequips: MessageWriter<'w, Intent<Unequip>>,
    drops: MessageWriter<'w, Intent<DropItem>>,
    uses: MessageWriter<'w, Intent<UseItem>>,
    uses_on: MessageWriter<'w, Intent<UseOn>>,
    aimed_uses: MessageWriter<'w, AimUse>,
    throws: MessageWriter<'w, AimThrow>,
    interacts: MessageWriter<'w, Intent<Interact>>,
    /// How many of a stack a drop asks, and where the bag is drawn, for the
    /// prompt to sit over it.
    counting: ResMut<'w, crate::panel::ground::Counting>,
    layout: Res<'w, InventoryLayout>,
}

/// Opens the bag choosing for a thing a game's own use key asked to use,
/// written as [`AimUse`] for anything, when its use asks for another of the
/// player's things: what `AimUse` means for such a thing, as the targeting
/// cursor answers it for one whose use is aimed.
///
/// Its own system because the bag's keys write `AimUse` for an aimed
/// thing, and one system may not read the messages it writes. Only with no
/// screen up, as a game's key opens nothing over another.
pub(crate) fn choose_when_asked(mut asked: MessageReader<AimUse>, view: Res<InventoryView>, mut menu: ResMut<InventoryMenu>, mut modals: ResMut<Modals>) {
    let modal = inventory_modal(&modals);
    for asked in asked.read() {
        let wants_a_thing = view.rows.iter().any(|r| r.entity == asked.item && r.asks.is_some() && r.usable());
        if view.entity == Some(asked.user) && wants_a_thing && !modals.any_open() {
            start_choosing(&mut menu, Choosing::Use(ChoosingFor { item: asked.item, selected: 0 }));
            modals.open(modal);
        }
    }
}

/// Opens the bag choosing what `taker` takes up `target`'s `verb` with, an
/// offer that chooses first, asking the game's `among` what may be chosen.
///
/// Queued as a command, since the game's system is run on the world and
/// the offers screen and the interact key that take the offer up are
/// ordinary systems. With nothing to choose among the bag stays shut and
/// nothing is taken up: the game's system, which knows why, says so if it
/// wants to, as only it can.
///
/// # Panics
/// When the game added no [`InventoryPanel`], which a choosing offer
/// cannot be taken up without, or its system is gone.
pub(crate) fn choose_for_offer(commands: &mut Commands, taker: Entity, target: Entity, verb: VerbId, prompt: String, among: OfferFilter) {
    commands.queue(move |world: &mut World| {
        assert!(world.contains_resource::<InventoryMenu>(), "play cannot go on: an offer chooses first, and the bag chooses for it; add InventoryPanel");
        let chooser = OfferChooser { taker, offerer: target, verb };
        let choices = world.run_system_with(among, chooser).unwrap_or_else(|e| panic!("an offer that chooses first named a system that could not run: {e}"));
        if choices.is_empty() {
            return;
        }
        let mut menu = world.resource_mut::<InventoryMenu>();
        start_choosing(&mut menu, Choosing::Offer(OfferChoice { target, verb, prompt, choices, selected: 0 }));
        let mut modals = world.resource_mut::<Modals>();
        let bag = inventory_modal(&modals);
        modals.open(bag);
    });
}

/// Opens, closes and walks the bag, opens a row, and acts on an opened
/// one.
///
/// An action is taken only while the player holds the turn, since an intent
/// written for anyone else is dropped unread; the screen stays up until it
/// does. A use that asks for another of the player's things turns the bag
/// to choosing among what it may be made on, under its prompt: confirm uses
/// it on the thing picked out, and putting the bag away spends nothing. An
/// offer that chooses first chooses the same way, and confirm takes it up
/// with the thing picked out.
pub(crate) fn inventory_keys(
    keys: ControlInput,
    binds: Res<InventoryKeys>,
    view: Res<InventoryView>,
    mut menu: ResMut<InventoryMenu>,
    mut modals: ResMut<Modals>,
    holding: Query<(), (With<Player>, With<MyTurn>)>,
    mut intents: BagIntents,
) {
    let modal = inventory_modal(&modals);
    // However the bag was put away, by its own keys or by a screen that
    // closed everything, it comes back on the list, choosing for nothing.
    if !modals.is_open(modal) {
        back_to_the_list(&mut menu);
    }
    let input = keys.input();
    // On a list every plain letter is a row's, the toggle's included, so
    // what a key does there never depends on how much is carried.
    let lettered = modals.is_top(modal) && (!menu.opened || menu.choosing.is_some());
    if binds.toggle.just_pressed(input) && !(lettered && a_letter(&binds.toggle)) && (modals.is_top(modal) || !modals.any_open()) {
        modals.toggle(modal);
        back_to_the_list(&mut menu);
        return;
    }
    // Not in the frame it went up: a game may open the bag on a key the
    // bag reads too, and that press is the game's.
    if !modals.is_top(modal) || modals.just_opened() {
        return;
    }
    let bindings = keys.bindings();
    if input.just_pressed(bindings.cursor.close) {
        if menu.opened {
            // Back to the list is the opened row closing, so it is recorded
            // on the stack as a close: the engine's catch-all for the close
            // key sees the press answered and leaves the bag up. The stack
            // is as it was, the bag on top.
            menu.opened = false;
            modals.close_one(modal);
            modals.open(modal);
        } else {
            modals.close_one(modal);
            back_to_the_list(&mut menu);
        }
        return;
    }
    let confirmed = input.just_pressed(bindings.cursor.confirm) || input.just_pressed(bindings.cursor.also_confirm);
    if let Some((_, choices, selected)) = choices_of(&menu, &view) {
        // A letter picks its choice and makes it, there being nothing else
        // to do with a choice; the use key confirms too, unless it is a
        // letter, which is a choice's here.
        let lettered = lettered_row(input).filter(|at| *at < choices.len());
        let confirmed = confirmed || lettered.is_some() || (binds.use_it.just_pressed(input) && !a_letter(&binds.use_it));
        let selected = lettered.unwrap_or_else(|| stepped(selected.min(choices.len().saturating_sub(1)), walked(&keys), choices.len()));
        choose_at(&mut menu, selected);
        let (Some(user), Some(&on)) = (view.entity, choices.get(selected)) else { return };
        if confirmed && holding.contains(user) {
            match &menu.choosing {
                // The offer taken up, checked by the engine as any is: an
                // offerer gone out of reach while the bag was up is
                // refused, for nothing.
                Some(Choosing::Offer(offer)) => {
                    intents.interacts.write(Intent::new(user, Interact { target: offer.target, verb: offer.verb, thing: Some(on) }));
                }
                Some(Choosing::Use(choosing)) => {
                    intents.uses_on.write(Intent::new(user, UseOn { item: choosing.item, on }));
                }
                None => {}
            }
            back_to_the_list(&mut menu);
            modals.close_all();
        }
        return;
    }
    if !menu.opened {
        pick(&mut menu, view.rows.len(), &keys);
        return;
    }
    let Some(row) = view.row(menu.selected) else {
        // The row went while it was open; the list is what is left.
        menu.opened = false;
        return;
    };
    let Some(user) = view.entity.filter(|user| holding.contains(*user)) else { return };
    let item = row.entity;
    let confirmed = confirmed || binds.use_it.just_pressed(input);
    // A worn thing that will not come off is offered no key that would take
    // it off, and the keys the footer leaves out do nothing to it.
    let acted = if binds.wear.just_pressed(input) {
        if row.worn() && !row.stuck {
            intents.unequips.write(Intent::new(user, Unequip(item)));
            true
        } else if !row.worn() && row.wearable() {
            intents.equips.write(Intent::new(user, Equip(item)));
            true
        } else {
            false
        }
    } else if binds.drop.just_pressed(input) && !row.stuck && row.count > 1 {
        // A stack asks how many, over the bag, which a drop closes.
        let rect = intents.layout.rect;
        intents.counting.ask(crate::panel::ground::CountFor::Drop, item, &row.label, row.count, modal, rect);
        let prompt = modals.get(crate::panel::ground::COUNT_MODAL).expect("the pack adds the count prompt");
        modals.open(prompt);
        false
    } else if binds.drop.just_pressed(input) && !row.stuck {
        intents.drops.write(Intent::new(user, DropItem::all(item)));
        true
    } else if confirmed {
        // What does something when used, with a charge to spend. Anything
        // else is not used from here: it would spend a turn on nothing, and
        // the footer never offers it. A use pointed somewhere closes the
        // bag and opens the targeting cursor in its place, as a throw does,
        // and one made on a thing keeps the bag up, choosing for it.
        match (row.usable(), row.targeting, row.asks.is_some()) {
            (true, _, true) => {
                start_choosing(&mut menu, Choosing::Use(ChoosingFor { item, selected: 0 }));
                false
            }
            (true, Some(_), false) => {
                intents.aimed_uses.write(AimUse { user, item });
                true
            }
            (true, None, false) => {
                intents.uses.write(Intent::new(user, UseItem(item)));
                true
            }
            (false, _, _) => false,
        }
    } else if binds.throw.just_pressed(input) && row.throw_range.is_some() && !row.stuck {
        // The bag closes and the targeting cursor opens in its place.
        intents.throws.write(AimThrow { user, item });
        true
    } else {
        false
    };
    if acted {
        // An action spends a turn, and the turn loop assumes no screen is
        // up when it runs.
        modals.close_all();
        back_to_the_list(&mut menu);
    }
}

/// Whether `chord` is a plain letter, and so a row's on a lettered list
/// before it is anything else.
fn a_letter(chord: &Chord) -> bool {
    !chord.shift && is_letter(chord.key)
}

/// The rows a direction key asks a lettered list to walk this frame: one
/// up, one down, or none.
///
/// A direction bound to a letter does not walk, pressed or held, since the
/// letter is a row's: the vi keys would otherwise open a row in a long bag
/// and walk a short one. A held key walks at
/// [`RepeatPace`](crate::RepeatPace), the pace the look cursor steps at.
pub(crate) fn walked(keys: &ControlInput) -> i32 {
    let (input, directions) = (keys.input(), &keys.bindings().directions.0);
    let pressed = directions.iter().find(|(key, _)| !is_letter(*key) && input.just_pressed(*key)).map(|(_, d)| *d);
    // A repeat is of the first direction key held, as `advance_repeats`
    // finds it, so that is the one that must not be a letter.
    let held_off_the_letters = directions.iter().find(|(key, _)| input.pressed(*key)).is_some_and(|(key, _)| !is_letter(*key));
    match pressed.or_else(|| keys.repeats().firing(false).filter(|_| held_off_the_letters)) {
        Some(Direction::North) => -1,
        Some(Direction::South) => 1,
        _ => 0,
    }
}

/// The list's keys over `rows` rows: a letter opens its row, the confirm
/// key opens the row under the cursor, and the direction keys walk.
fn pick(menu: &mut InventoryMenu, rows: usize, keys: &ControlInput) {
    let (input, bindings) = (keys.input(), keys.bindings());
    if let Some(row) = lettered_row(input).filter(|row| *row < rows) {
        menu.open(row);
        return;
    }
    if rows > 0 && (input.just_pressed(bindings.cursor.confirm) || input.just_pressed(bindings.cursor.also_confirm)) {
        menu.open(menu.selected.min(rows - 1));
        return;
    }
    menu.move_by(walked(keys), rows);
}

/// The letters a list of `rows` rows offers, `a-c` for three, and nothing
/// for none; past `z` the cursor reaches the rest.
fn letters(rows: usize) -> Option<String> {
    match rows.min(26).checked_sub(1).and_then(row_letter)? {
        'a' => Some("a".to_string()),
        last => Some(format!("a-{last}")),
    }
}

/// What the footer offers: on the list, the letters there are and the key
/// that opens the row under the cursor; on an opened row, the keys that do
/// something to it and the one that goes back.
///
/// Only what applies, so the footer never offers to wear a pebble, or to
/// take off, drop or throw a worn thing that will not come off. Short
/// enough for a narrow frame: the direction keys walk the rows on every
/// screen and need no saying here.
fn hints(view: &InventoryView, menu: &InventoryMenu, binds: &InventoryKeys, confirm: String, close: String) -> String {
    let mut keys = Vec::new();
    if let Some(row) = view.row(menu.selected).filter(|_| menu.opened) {
        if row.worn() && !row.stuck {
            keys.push(format!("{} take off", binds.wear.label()));
        } else if !row.worn() && row.wearable() {
            keys.push(format!("{} wear", binds.wear.label()));
        }
        if !row.stuck {
            keys.push(format!("{} drop", binds.drop.label()));
        }
        if row.usable() {
            keys.push(format!("{confirm} use"));
        }
        if row.throw_range.is_some() && !row.stuck {
            keys.push(format!("{} throw", binds.throw.label()));
        }
        keys.push(format!("{close} back"));
        return keys.join(" \u{2022} ");
    }
    if let Some(letters) = letters(view.rows.len()) {
        keys.push(format!("{letters} {confirm} open"));
    }
    keys.push(format!("{close} close"));
    keys.join(" \u{2022} ")
}

/// A strike in words: `1d6 cutlass`, or `1d8 pistol to 6` with a reach,
/// and `1d4+1d2 kinetic` with the wielder's own blow beside its weapon's.
/// Shared with the look panel, so a thing reads the same on the floor as
/// in the bag.
pub(crate) fn strike(s: &Strike) -> String {
    match s.range {
        Some(range) => format!("{} {} to {range}", rolls(s), s.kind),
        None => format!("{} {}", rolls(s), s.kind),
    }
}

/// What a strike rolls, as one sum: its roll alone, `1d4+1`, or with the
/// roll beside it in the same hit, the dice of both and then every bonus
/// added up, `1d4+1d2+1`, so it reads as the one hit it lands as.
pub(crate) fn rolls(s: &Strike) -> String {
    let Some(also) = s.also else { return s.dice.to_string() };
    let dice: Vec<String> = [s.dice, also].iter().filter(|r| r.num != 0 && r.sides != 0).map(|r| format!("{}d{}", r.num, r.sides)).collect();
    let bonus = s.dice.bonus + also.bonus;
    match (dice.is_empty(), bonus.cmp(&0)) {
        (true, _) => bonus.to_string(),
        (false, std::cmp::Ordering::Equal) => dice.join("+"),
        (false, std::cmp::Ordering::Greater) => format!("{}+{bonus}", dice.join("+")),
        (false, std::cmp::Ordering::Less) => format!("{}{bonus}", dice.join("+")),
    }
}

/// The lines describing `row` to fit `width`: the thing itself, then what
/// putting it on would change. Shared with the trade screen, so a ware on
/// a counter reads as it would in the bag.
///
/// `own` is what the wearer's own blow and shot are called where gear is
/// weighed, the game's word: `fists`, or `own` where it gives none.
pub(crate) fn describe(row: &ItemRow, width: usize, own: &str) -> Vec<(String, ToneId)> {
    let mut lines = about(row, width, true);
    lines.extend(compare(row, width, own));
    lines
}

/// The lines describing `row` itself: its numbers and what the game added,
/// then where it goes and whether it will come off, then what it does and
/// has left. A thing not yet known says so in place of its numbers and of
/// what it does and has left, and is described by the rest.
///
/// Not `whole`, the lines that tell one thing from another of its kind
/// alone: none of what the game added nor where it is worn, which two
/// things alike may differ in.
fn about(row: &ItemRow, width: usize, whole: bool) -> Vec<(String, ToneId)> {
    let mut lines: Vec<(String, ToneId)> = Vec::new();
    let mut say = |text: String, tone: ToneId| lines.extend(wrap(&text, width).into_iter().map(|l| (l, tone)));
    // What the game says a thing not yet known seems, its kind known and
    // not its level, say, and the bare word where it says nothing.
    if row.unknown && row.seems.is_empty() {
        say("unknown".to_string(), Tones::MUTED);
    }
    if row.unknown {
        for seems in &row.seems {
            say(seems.text.clone(), seems.tone);
        }
    }
    if let Some(blow) = &row.blow {
        say(strike(blow), Tones::TEXT);
    }
    if let Some(shot) = &row.shot {
        say(format!("shoots {}", strike(shot)), Tones::TEXT);
    }
    for extra in &row.strikes {
        say(format!("+{}", strike(extra)), Tones::TEXT);
    }
    if let Some(launch) = &row.launches {
        say(format!("launches {} {} {} to {}", launch.ammo, launch.roll, launch.kind, launch.range), Tones::TEXT);
    }
    if let Some(tiles) = row.dark_sight {
        say(format!("sees {tiles} in the dark"), Tones::TEXT);
    }
    if row.armor != 0 {
        say(format!("armor {:+}", row.armor), Tones::TEXT);
    }
    for r in &row.resists {
        match r.pct {
            pct if pct < 0 => say(format!("weak to {} {}%", r.name, -pct), Tones::TEXT),
            pct => say(format!("resists {} {pct}%", r.name), Tones::TEXT),
        }
    }
    for bestowed in &row.bestows {
        say(format!("{} {}", bestowed.stat, plain_op(bestowed.op)), Tones::TEXT);
    }
    match (&row.thrown, row.throw_range) {
        (Some(thrown), _) => say(format!("thrown {}", strike(thrown)), Tones::TEXT),
        (None, Some(range)) if !row.unknown => say(format!("thrown to {range}"), Tones::TEXT),
        (None, _) => {}
    }
    // What the game added is a fact about the thing the engine could not
    // know, a sight it lends or why a gun cannot fire, so it reads with the
    // numbers rather than under what the thing does and has left.
    for facet in row.facets.iter().filter(|_| whole) {
        say(facet.text.clone(), facet.tone);
    }
    // A two-hander names the hand it empties as well, since putting it on
    // takes whatever is there off.
    let also = row.also_takes.iter().map(|slot| format!("the {slot}")).collect::<Vec<_>>().join(" and ");
    if !whole {
    } else if row.worn() {
        let also = if also.is_empty() { also } else { format!(" and {also}") };
        say(format!("worn on the {}{also}", row.slot_name), Tones::MUTED);
    } else if row.wearable() {
        let also = if also.is_empty() { also } else { format!(", and takes {also}") };
        say(format!("goes on the {}{also}", row.goes_on.join(" or the ")), Tones::MUTED);
    }
    // Why the footer offers nothing that would take it off.
    if row.stuck && whole {
        say("will not come off".to_string(), Tones::BAD);
    }
    if row.unknown {
        return lines;
    }
    for what in &row.used {
        say(what.clone(), Tones::TEXT);
    }
    // How far a use reaches and what it wants there, in the ability
    // list's own words.
    if let Some(targeting) = row.targeting {
        say(format!("aimed {}", reach(targeting.aim, targeting.mode)), Tones::MUTED);
    }
    if let Some((left, max)) = row.charges {
        say(format!("{left}/{max} charges"), Tones::MUTED);
    }
    // An attuned thing off the body, charged or not, says the whole rule:
    // a charge it holds is lost going back on, which is the one thing a
    // player swapping plates needs to know. One whose clock runs says when
    // it will be ready; anything else empty is simply empty.
    match (row.attuned && !row.worn(), row.empty, row.ready_in) {
        (true, _, _) => say("empties when put on; charges only while worn".to_string(), Tones::MUTED),
        (false, true, Some(whole)) => say(format!("ready in {}", turns(whole * atglyph_core::turn::BASE_ACTION_COST)), Tones::MUTED),
        (false, true, None) => say("empty".to_string(), Tones::MUTED),
        (false, false, _) => {}
    }
    lines
}

/// What putting `row` on would change, under what it is: what it replaces
/// in the slot it goes in, or that nothing is worn there; what else it
/// takes, and from what; the slots it leaves bare; and each number that
/// would change, old to new and by how much, a gain in the good tone and a
/// loss in the bad. Nothing for a thing worn or that cannot be.
///
/// A thing that will not come off stands in the way, and is said to in
/// place of the numbers, since wearing it is refused. Neither is any
/// number compared while either side is not yet known: the numbers of a
/// thing unknown are withheld, and a difference with nothing would be a
/// guess passed off as a reading.
fn compare(row: &ItemRow, width: usize, own: &str) -> Vec<(String, ToneId)> {
    let Some(replacing) = &row.replacing else { return Vec::new() };
    let mut lines: Vec<(String, ToneId)> = Vec::new();
    let mut say = |text: String, tone: ToneId| lines.extend(wrap(&text, width).into_iter().map(|l| (l, tone)));
    let (in_slot, beside): (Vec<&TakenOff>, Vec<&TakenOff>) = replacing.off.iter().partition(|t| t.from.contains(&replacing.slot));
    if in_slot.is_empty() {
        say(format!("nothing worn on the {}", replacing.slot), Tones::MUTED);
    } else {
        say(format!("replaces: {}", in_slot.iter().map(|t| t.row.label.as_str()).collect::<Vec<_>>().join(" and ")), Tones::TEXT);
    }
    for taken in beside {
        say(format!("takes the {} from {}", taken.from.join(" and the "), atglyph_core::noun::with_the(&taken.row.label)), Tones::TEXT);
    }
    if !replacing.frees.is_empty() {
        say(format!("frees the {}", replacing.frees.join(" and the ")), Tones::MUTED);
    }
    let stuck: Vec<&TakenOff> = replacing.off.iter().filter(|t| t.row.stuck).collect();
    for taken in &stuck {
        say(format!("{} will not come off", atglyph_core::noun::with_the(&taken.row.label)), Tones::BAD);
    }
    if !stuck.is_empty() || row.unknown || replacing.off.iter().any(|t| t.row.unknown) {
        return lines;
    }
    let off: Vec<&ItemRow> = replacing.off.iter().map(|t| &t.row).collect();
    let changes = changes(row, &off, replacing, own);
    // Something taken off for its like changes nothing, and says so
    // rather than leaving a reader to wonder whether the numbers are cut;
    // a thing that differs in what is not weighed says nothing.
    if let [only] = off.as_slice()
        && changes.is_empty()
        && about(row, width, false) == about(only, width, false)
    {
        lines.push(("  no change".to_string(), Tones::MUTED));
    }
    lines.extend(table(&changes, width));
    lines
}

/// One number putting a thing on would change: what it is, the old and
/// the new in words, the difference in halves so a roll's mean of 2.5 is
/// exact, what the difference is counted in, and how much better it is,
/// in halves, which is the difference unless less is better, and `None`
/// for a change neither better nor worse, such as a launcher for
/// something else launched.
struct Change {
    what: String,
    old: String,
    new: String,
    by: Option<i64>,
    unit: &'static str,
    gain: Option<i64>,
}

/// What an average of rolls is said in.
const AVERAGE: &str = " avg";

/// Twice the mean of `roll`, so a mean of 2.5 is a whole number.
fn halves(roll: atglyph_core::DiceRoll) -> i64 {
    let dice = if roll.num == 0 || roll.sides == 0 { 0 } else { roll.num as i64 * (roll.sides as i64 + 1) };
    dice + 2 * roll.bonus as i64
}

/// Twice the mean of what `s` lands in one hit: its roll and the roll
/// beside it.
fn strike_halves(s: &Strike) -> i64 {
    halves(s.dice) + s.also.map_or(0, halves)
}

/// A difference in halves as a signed number: `+1`, `-2.5`.
fn signed_halves(halves: i64) -> String {
    let sign = if halves < 0 { '-' } else { '+' };
    match halves.abs() {
        whole if whole % 2 == 0 => format!("{sign}{}", whole / 2),
        half => format!("{sign}{}.5", half / 2),
    }
}

/// Every number that putting `row` on in place of `off` would change, in
/// the order the bag describes them: armor, the blow, the shot, the extra
/// rolls, what each launcher shoots, what is resisted, sight in the dark,
/// and the stats. What stays the same is left out.
///
/// What adds up is weighed thing against thing: armor, extra rolls,
/// resistances and stats, which the wearer has the sum of. What one thing
/// decides is weighed by what the wearer would have before and after, as
/// a fight reads it: the blow and the shot of the first worn thing that
/// has one, else the wearer's own, called `own`; the first launcher for
/// each thing launched; the best sight in the dark.
fn changes(row: &ItemRow, off: &[&ItemRow], replacing: &Replacing, own: &str) -> Vec<Change> {
    let mut changes = Vec::new();
    // A change toned by its difference, better the more it rises.
    let change = |changes: &mut Vec<Change>, what: &str, old: String, new: String, by: Option<i64>, unit: &'static str, toned: bool| {
        if old != new {
            changes.push(Change { what: what.to_string(), old, new, by, unit, gain: by.filter(|_| toned) });
        }
    };
    let none = || "none".to_string();
    let worn_armor: i32 = off.iter().map(|r| r.armor).sum();
    if worn_armor != 0 || row.armor != 0 {
        change(&mut changes, "armor", worn_armor.to_string(), row.armor.to_string(), Some(2 * (row.armor - worn_armor) as i64), "", true);
    }
    let (before, after) = (&replacing.before, &replacing.after);
    let weighed = !before.withheld && !after.withheld;
    // A blow or a shot is said with what it comes from, and its kind only
    // where the kinds differ.
    for (what, old, new) in [("blow", &before.blow, &after.blow), ("shot", &before.shot, &after.shot)].into_iter().filter(|_| weighed) {
        let kinds_differ = matches!((old, new), (Some(a), Some(b)) if a.strike.kind != b.strike.kind);
        let said = |w: &Option<Wielding>| match w {
            None => none(),
            Some(w) => {
                let from = w.from.as_deref().unwrap_or(own);
                let dice = if kinds_differ { format!("{} {}", rolls(&w.strike), w.strike.kind) } else { rolls(&w.strike) };
                match w.strike.range {
                    Some(range) => format!("{from} {dice} to {range}"),
                    None => format!("{from} {dice}"),
                }
            }
        };
        let mean = |w: &Option<Wielding>| w.as_ref().map_or(0, |w| strike_halves(&w.strike));
        change(&mut changes, what, said(old), said(new), Some(mean(new) - mean(old)), AVERAGE, true);
    }
    let extras = |rows: &mut dyn Iterator<Item = &Strike>| -> (String, i64) {
        let all: Vec<&Strike> = rows.collect();
        let said = all.iter().map(|s| format!("+{}", strike(s))).collect::<Vec<_>>().join(", ");
        (if said.is_empty() { none() } else { said }, all.iter().map(|s| strike_halves(s)).sum())
    };
    let (old_extra, old_mean) = extras(&mut off.iter().flat_map(|r| &r.strikes));
    let (new_extra, new_mean) = extras(&mut row.strikes.iter());
    change(&mut changes, "extra", old_extra, new_extra, Some(new_mean - old_mean), AVERAGE, true);
    // Each thing launched, as the first launcher worn for it launches it.
    // A launcher put on for something else launched is neither better nor
    // worse than the one it replaces, only other.
    let mut ammo: Vec<&str> = Vec::new();
    for line in before.launchers.iter().chain(&after.launchers).filter(|_| weighed) {
        if !ammo.contains(&line.ammo.as_str()) {
            ammo.push(&line.ammo);
        }
    }
    let other = |kit: &Kit, ammo: &str| kit.launchers.iter().any(|l| l.ammo != ammo);
    for ammo in ammo {
        let (old, new) = (before.launchers.iter().find(|l| l.ammo == ammo), after.launchers.iter().find(|l| l.ammo == ammo));
        let toned = match (old, new) {
            (Some(_), None) => !other(after, ammo),
            (None, Some(_)) => !other(before, ammo),
            _ => true,
        };
        let roll = |l: Option<&LaunchLine>| l.map_or_else(none, |l| l.roll.to_string());
        let mean = |l: Option<&LaunchLine>| l.map_or(0, |l| halves(l.roll));
        change(&mut changes, &format!("{ammo} roll"), roll(old), roll(new), Some(mean(new) - mean(old)), AVERAGE, toned);
        let range = |l: Option<&LaunchLine>| l.map_or_else(none, |l| l.range.to_string());
        let cells = |l: Option<&LaunchLine>| l.map_or(0, |l| i64::from(l.range));
        change(&mut changes, &format!("{ammo} range"), range(old), range(new), Some(2 * (cells(new) - cells(old))), "", toned);
    }
    // Each kind resisted on either side, in the order they are first met.
    let mut kinds: Vec<&str> = Vec::new();
    for line in row.resists.iter().chain(off.iter().flat_map(|r| &r.resists)) {
        if !kinds.contains(&line.name.as_str()) {
            kinds.push(&line.name);
        }
    }
    for kind in kinds {
        let pct = |rows: &mut dyn Iterator<Item = &ResistLine>| -> i32 { rows.filter(|r| r.name == kind).map(|r| r.pct).sum() };
        let (old, new) = (pct(&mut off.iter().flat_map(|r| &r.resists)), pct(&mut row.resists.iter()));
        change(&mut changes, &format!("resists {kind}"), format!("{old}%"), format!("{new}%"), Some(2 * (new - old) as i64), "", true);
    }
    if weighed {
        let said = |tiles: Option<i32>| tiles.map_or_else(none, |t| t.to_string());
        let gain = after.dark_sight.unwrap_or(0) - before.dark_sight.unwrap_or(0);
        change(&mut changes, "dark sight", said(before.dark_sight), said(after.dark_sight), Some(2 * gain as i64), "", true);
    }
    // Each stat moved on either side: a sum where every move is an add,
    // toned by which way the stat is better, and the moves themselves, no
    // better or worse, where any is not.
    let mut stats: Vec<&Bestowed> = Vec::new();
    for bestowed in row.bestows.iter().chain(off.iter().flat_map(|r| &r.bestows)) {
        if !stats.iter().any(|b| b.stat == bestowed.stat) {
            stats.push(bestowed);
        }
    }
    for stat in stats {
        let ops = |rows: &mut dyn Iterator<Item = &Bestowed>| -> Vec<Op> { rows.filter(|b| b.stat == stat.stat).map(|b| b.op).collect() };
        let (old, new) = (ops(&mut off.iter().flat_map(|r| &r.bestows)), ops(&mut row.bestows.iter()));
        let sum = |ops: &[Op]| -> Option<i32> { ops.iter().map(|op| if let Op::Add(n) = op { Some(*n) } else { None }).sum() };
        match (sum(&old), sum(&new)) {
            (Some(was), Some(will)) => {
                change(&mut changes, &stat.stat, format!("{was:+}"), format!("{will:+}"), Some(2 * (will - was) as i64), "", true);
                if let Some(last) = changes.last_mut().filter(|c| c.what == stat.stat && stat.lower_is_better) {
                    last.gain = last.gain.map(|gain| -gain);
                }
            }
            _ => {
                let said = |ops: &[Op]| if ops.is_empty() { none() } else { ops.iter().map(|op| plain_op(*op)).collect::<Vec<_>>().join(", ") };
                change(&mut changes, &stat.stat, said(&old), said(&new), None, "", false);
            }
        }
    }
    changes
}

/// `changes` as lines under what a thing replaces, set in columns, the
/// difference after each in brackets, where the widest fits `width`, and
/// each said plainly and wrapped where it does not.
fn table(changes: &[Change], width: usize) -> Vec<(String, ToneId)> {
    let widest = |of: fn(&Change) -> &str| changes.iter().map(|c| of(c).chars().count()).max().unwrap_or(0);
    let (what, old, new) = (widest(|c| &c.what), widest(|c| &c.old), widest(|c| &c.new));
    let diff = |c: &Change| c.by.filter(|h| *h != 0).map(|h| format!(" ({}{})", signed_halves(h), c.unit)).unwrap_or_default();
    let columns = |c: &Change| format!("  {:<what$} {:<old$} -> {:<new$}{}", c.what, c.old, c.new, diff(c)).trim_end().to_string();
    let aligned = changes.iter().all(|c| columns(c).chars().count() <= width);
    let mut lines = Vec::new();
    for c in changes {
        let tone = match c.gain {
            Some(gain) if gain > 0 => Tones::GOOD,
            Some(gain) if gain < 0 => Tones::BAD,
            _ => Tones::TEXT,
        };
        if aligned {
            lines.push((columns(c), tone));
        } else {
            let plain = format!("{} {} -> {}{}", c.what, c.old, c.new, diff(c));
            lines.extend(wrap(&plain, width.saturating_sub(2)).into_iter().map(|l| (format!("  {l}"), tone)));
        }
    }
    lines
}

/// Prints `lines` down from row `y` to `bottom` at `x`, each clipped to
/// `width`, on the surface. Lines past the last row are marked by an
/// ellipsis in the muted tone at the end of the last, since words cut
/// short without a mark read as all there is.
pub(crate) fn print_lines(terminal: &mut Terminal, (x, y, bottom): (i32, i32, i32), width: usize, lines: &[(String, ToneId)], palette: &Palette) {
    let room = (bottom - y).max(0) as usize;
    let cut = lines.len() > room;
    for (i, (line, tone)) in lines.iter().take(room).enumerate() {
        let last = cut && i + 1 == room;
        let text = if last { clip(line, width.saturating_sub(2)) } else { clip(line, width) };
        terminal.print_on(x, y + i as i32, &text, palette.get(*tone), palette.get(Tones::SURFACE));
        if last && width > 0 {
            terminal.print_on(x + width as i32 - 1, y + i as i32, "\u{2026}", palette.get(Tones::MUTED), palette.get(Tones::SURFACE));
        }
    }
}

/// What the bag is drawn from.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct BagScreen<'w> {
    layout: Res<'w, InventoryLayout>,
    view: Res<'w, InventoryView>,
    binds: Res<'w, InventoryKeys>,
    keys: ControlInput<'w>,
    modals: Res<'w, Modals>,
    palette: Res<'w, Palette>,
}

/// Paints the bag while its modal is open: the rows, a rule, and the row
/// picked out described under it. Choosing what a use is made on, it is
/// titled with the use's prompt and lists only what it may be made on.
/// Rows past the list's room are counted on the frame above it and the
/// rule below, since a bag cut short without a word reads as a short bag.
pub(crate) fn draw_inventory(mut terminal: ResMut<Terminal>, mut menu: ResMut<InventoryMenu>, screen: BagScreen, mut top: Local<usize>) {
    let BagScreen { layout, view, binds, keys, modals, palette } = &screen;
    if !modals.is_open(inventory_modal(modals)) {
        return;
    }
    let rect = layout.rect;
    if rect.width < 12 || rect.height < 6 {
        return;
    }
    let bindings = keys.bindings();
    let (confirm, close) = (key_name(bindings.cursor.confirm), key_name(bindings.cursor.close));
    // The cursor survives the rebuild, clamped to the rows there are now,
    // before the footer asks which row it is on.
    let (title, rows, at, hints) = match choices_of(&menu, view) {
        Some((prompt, choices, selected)) => {
            let rows: Vec<&ItemRow> = choices.iter().filter_map(|on| view.rows.iter().find(|r| r.entity == *on)).collect();
            let at = selected.min(rows.len().saturating_sub(1));
            choose_at(&mut menu, at);
            let letters = letters(rows.len()).map(|l| format!("{l} ")).unwrap_or_default();
            (prompt, rows, at, format!("{letters}{confirm} choose \u{2022} {close} {}", layout.never_mind))
        }
        None => {
            menu.selected = menu.selected.min(view.rows.len().saturating_sub(1));
            (layout.title.clone(), view.rows.iter().collect(), menu.selected, hints(view, &menu, binds, confirm, close))
        }
    };
    clear(&mut terminal, rect, palette);
    frame(&mut terminal, rect, &title, &hints, palette);
    let inner = GridRect::new(rect.x + 2, rect.y + 1, rect.width - 4, rect.height - 2);
    let surface = palette.get(Tones::SURFACE);
    let width = inner.width.max(0) as usize;

    if rows.is_empty() {
        terminal.print_on(inner.x, inner.y, &clip(&layout.empty, width), palette.get(Tones::MUTED), surface);
        return;
    }
    // Half the height to the rows, the rest to the words about the one
    // picked out, and never fewer than three rows.
    let list_rows = ((inner.height / 2) as usize).max(3).min(rows.len());
    // The list moves only as far as keeps the row picked out in it, so it
    // holds still under a cursor moving inside it.
    *top = (*top).min(at).max((at + 1).saturating_sub(list_rows)).min(rows.len() - list_rows);
    let first = *top;
    for (i, row) in rows.iter().enumerate().skip(first).take(list_rows) {
        let y = inner.y + (i - first) as i32;
        let selected = i == at;
        let bg = if selected { palette.get(Tones::SELECT) } else { surface };
        // With a row opened, the rest of the list steps back, so the keys
        // along the bottom read as that row's.
        let tone = match (menu.opened && !selected, row.worn()) {
            (true, _) => Tones::MUTED,
            (false, true) => Tones::NOTICE,
            (false, false) => Tones::TEXT,
        };
        let fg = palette.get(tone);
        terminal.fill(GridRect::new(inner.x, y, inner.width, 1), Cell::new(' ', fg).on(bg));
        // The row's letter, then the item's own glyph in its own colour,
        // then its name; a stack says how many. A row past the letters
        // keeps their room, so every name starts in one column.
        if let Some(letter) = row_letter(i) {
            terminal.print_on(inner.x, y, &format!("{letter})"), fg, bg);
        }
        let mut x = inner.x + 3;
        if let Some(glyph) = row.glyph {
            terminal.set(x, y, Cell::new(glyph.ch, glyph.fg).on(bg));
            x += 2;
        }
        let label = atglyph_core::noun::listed(&row.label, row.count);
        let tag = if row.worn() { row.slot_name.clone() } else { String::new() };
        let room = (inner.right() - x).max(0) as usize;
        terminal.print_on(x, y, &clip(&label, room.saturating_sub(tag.chars().count() + 1)), fg, bg);
        if !tag.is_empty() {
            terminal.print_on(inner.right() - tag.chars().count() as i32, y, &tag, palette.get(Tones::MUTED), bg);
        }
    }
    let rule_y = inner.y + list_rows as i32;
    for x in inner.x..inner.right() {
        terminal.put(x, rule_y, '\u{2500}', palette.get(Tones::FRAME));
    }
    // How many rows are out of view, on the line over the list and the
    // rule under it, so a long bag does not read as a short one. Both end
    // a cell short of the rule's end, so they line up and the rule keeps
    // its end, and the one above stays clear of the title.
    let titled = if title.is_empty() { rect.x + 1 } else { rect.x + 2 + format!(" {title} ").chars().count() as i32 };
    more(&mut terminal, rect.y, titled, inner.right() - 1, '\u{25b2}', first, palette);
    more(&mut terminal, rule_y, inner.x, inner.right() - 1, '\u{25bc}', rows.len() - first - list_rows, palette);
    let Some(row) = rows.get(at) else { return };
    print_lines(&mut terminal, (inner.x, rule_y + 1, inner.bottom()), width, &describe(row, width, &layout.own), palette);
}

/// Says on row `y` that `count` rows are out of view, `arrow` pointing to
/// them, in the muted tone, ending just short of `end` and starting past
/// `after`: with the count where there is room, the arrow alone where
/// there is not, and nothing where there is no room even for that.
fn more(terminal: &mut Terminal, y: i32, after: i32, end: i32, arrow: char, count: usize, palette: &Palette) {
    if count == 0 {
        return;
    }
    let said = [format!(" {arrow} {count} more "), format!(" {arrow} ")];
    let Some(text) = said.iter().find(|t| end - t.chars().count() as i32 > after) else { return };
    terminal.print_on(end - text.chars().count() as i32, y, text, palette.get(Tones::MUTED), palette.get(Tones::SURFACE));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;
    use atglyph_bevy::EngineApp;
    use atglyph_bevy::{Intent, ThrowingPlugin};
    use atglyph_core::DiceRoll;
    use atglyph_rules::Names;
    use atglyph_rules::content::Registry;
    use atglyph_rules::{EquipShape, Equipment, SlotDef, SlotId};

    #[test]
    fn a_strike_reads_its_roll_kind_and_reach() {
        let shot = Strike { kind: "kinetic".into(), dice: atglyph_core::DiceRoll::new(1, 2), range: Some(9), also: None };
        assert_eq!(strike(&shot), "1d2 kinetic to 9");
        assert_eq!(strike(&Strike { range: None, ..shot }), "1d2 kinetic", "a blow reaches nowhere");
    }

    /// A blow with a roll beside it in the same hit reads as one sum, the
    /// dice first and every bonus added up at the end.
    #[test]
    fn a_strike_with_its_own_blow_beside_it_reads_as_one_sum() {
        let blow = Strike { kind: "kinetic".into(), dice: DiceRoll::new(1, 4), range: None, also: Some(DiceRoll::new(1, 2)) };
        assert_eq!(strike(&blow), "1d4+1d2 kinetic");
        assert_eq!(strike(&Strike { dice: DiceRoll { bonus: 1, ..DiceRoll::new(1, 4) }, ..blow.clone() }), "1d4+1d2+1 kinetic", "the bonus at the end");
        assert_eq!(strike(&Strike { dice: DiceRoll::flat(5), also: Some(DiceRoll::flat(2)), ..blow }), "7 kinetic", "and two flat rolls one number");
    }

    /// A wearer that adds its own blow to a worn one weighs a dagger as the
    /// dagger's roll and its own together, not as the dagger in place of
    /// its own.
    #[test]
    fn a_blade_for_a_wearer_that_adds_its_own_blow_is_weighed_with_that_blow_beside_it() {
        let mut stage = closet(52, 22);
        let (right, kind, player) = (slot(&stage, "right hand"), stage.kind, stage.player);
        let w = stage.app.world_mut();
        w.entity_mut(player).insert((MeleeAttack::new(kind, DiceRoll::new(1, 2)), atglyph_bevy::AddsOwnBlow));
        let dagger = w.spawn((Item, Name::new("dagger"), Wearable(EquipShape::in_slot(right)), MeleeAttack::new(kind, DiceRoll::new(1, 4)))).id();
        dress(&mut stage, vec![dagger], &[], dagger);
        let blow = squeezed(&described(&stage)).into_iter().find(|(t, _)| t.trim_start().starts_with("blow "));
        assert_eq!(blow, Some(("  blow own 1d2 -> dagger 1d4+1d2 (+2.5 avg)".to_string(), Tones::GOOD)));
    }

    /// A player with a worn blade, a hat in the bag and a stack of knives,
    /// and the bag drawn in a small frame.
    fn staged() -> (Stage, Entity, Entity, Entity) {
        let mut stage =
            Stage::new_with((ThrowingPlugin, crate::TargetViewPlugin, InventoryPanel::new(GridRect::new(0, 0, 52, 14)).title("Bag").empty("Lint.")), |app| {
                app.world_mut().resource_mut::<Registries>().slots = Registry::from_defs(vec![SlotDef::new("hand"), SlotDef::new("head")]).unwrap();
            })
            .screen(52, 14);
        let (player, kind) = (stage.player, stage.kind);
        let (hand, head) = {
            let r = stage.app.world().resource::<Registries>();
            (r.slots.expect("hand"), r.slots.expect("head"))
        };
        let blade = stage
            .app
            .world_mut()
            .spawn((
                Item,
                Name::new("a blade"),
                atglyph_render::Glyph::new(')', Color::WHITE),
                Wearable(EquipShape::in_slot(hand)),
                MeleeAttack::new(kind, DiceRoll::new(1, 6)),
            ))
            .id();
        let mut resists = atglyph_rules::Resistances::new();
        resists.set(kind, 10);
        let hat = stage.app.world_mut().spawn((Item, Name::new("a hat"), Wearable(EquipShape::in_slot(head)), Armor(1), Resists(resists))).id();
        let knives = stage
            .app
            .world_mut()
            .spawn((Item, Name::new("knife"), Stack { key: 1, count: 3 }, Throwable::new(Some(5), Some((kind, DiceRoll::new(1, 4))))))
            .id();
        let mut worn = Equipped(Equipment::with_slot_count(2));
        worn.equip(blade, &EquipShape::in_slot(hand)).unwrap();
        stage.app.world_mut().entity_mut(player).insert((Inventory { items: vec![blade, hat, knives] }, worn));
        stage.tick();
        (stage, blade, hat, knives)
    }

    /// A two-hander says the second slot it takes, in the bag and worn,
    /// since putting it on empties that hand too.
    #[test]
    fn a_thing_that_takes_a_second_slot_says_so_in_the_bag_and_worn() {
        let mut stage = Stage::new_with(InventoryPanel::new(GridRect::new(0, 0, 52, 14)).title("Bag"), |app| {
            app.world_mut().resource_mut::<Registries>().slots = Registry::from_defs(vec![SlotDef::new("main hand"), SlotDef::new("off hand")]).unwrap();
        })
        .screen(52, 14);
        let (player, kind) = (stage.player, stage.kind);
        let (main, off) = {
            let r = stage.app.world().resource::<Registries>();
            (r.slots.expect("main hand"), r.slots.expect("off hand"))
        };
        let shape = EquipShape::in_slot(main).and_claims(off);
        let axe = stage.app.world_mut().spawn((Item, Name::new("an axe"), Wearable(shape.clone()), MeleeAttack::new(kind, DiceRoll::new(2, 6)))).id();
        stage.app.world_mut().entity_mut(player).insert((Inventory { items: vec![axe] }, Equipped(Equipment::with_slot_count(2))));
        stage.tick();
        stage.press(KeyCode::KeyI);
        assert_eq!(inside(&stage, 4), "goes on the main hand, and takes the off hand");

        stage.app.world_mut().get_mut::<Equipped>(player).unwrap().equip(axe, &shape).unwrap();
        stage.tick();
        assert_eq!(inside(&stage, 4), "worn on the main hand and the off hand");
    }

    /// A player wearing a cap on the head and a dagger in the left hand,
    /// carrying a helm, a bow that goes in the right hand and takes the
    /// left, a scarf for the bare neck and a pebble, with the bag drawn
    /// tall enough to say what each would change.
    fn wardrobe() -> (Stage, Entity, Entity) {
        let mut stage = Stage::new_with(InventoryPanel::new(GridRect::new(0, 0, 52, 22)).title("Bag"), |app| {
            let mut registries = app.world_mut().resource_mut::<Registries>();
            registries.slots =
                Registry::from_defs(vec![SlotDef::new("right hand"), SlotDef::new("left hand"), SlotDef::new("head"), SlotDef::new("neck")]).unwrap();
            registries.tags = Registry::from_defs(vec![atglyph_rules::TagDef::new("arrow")]).unwrap();
        })
        .screen(52, 22);
        let (player, kind) = (stage.player, stage.kind);
        let (right, left, head, neck, arrow) = {
            let r = stage.app.world().resource::<Registries>();
            (r.slots.expect("right hand"), r.slots.expect("left hand"), r.slots.expect("head"), r.slots.expect("neck"), r.tags.expect("arrow"))
        };
        let w = stage.app.world_mut();
        let cap = w.spawn((Item, Name::new("cap"), Wearable(EquipShape::in_slot(head)), Armor(1))).id();
        let dagger = w.spawn((Item, Name::new("dagger"), Wearable(EquipShape::in_slot(left)), MeleeAttack::new(kind, DiceRoll::new(1, 4)))).id();
        let helm = w.spawn((Item, Name::new("helm"), Wearable(EquipShape::in_slot(head)), Armor(2))).id();
        let launcher = atglyph_bevy::Launcher { ammo: arrow, effective: 2, range: 4, strike: (kind, DiceRoll::new(1, 4)) };
        let bow = w.spawn((Item, Name::new("bow"), Wearable(EquipShape::in_slot(right).and_claims(left)), launcher)).id();
        let scarf = w.spawn((Item, Name::new("scarf"), Wearable(EquipShape::in_slot(neck)), Armor(1))).id();
        let pebble = w.spawn((Item, Name::new("pebble"))).id();
        let mut worn = Equipped(Equipment::with_slot_count(4));
        worn.equip(cap, &EquipShape::in_slot(head)).unwrap();
        worn.equip(dagger, &EquipShape::in_slot(left)).unwrap();
        w.entity_mut(player).insert((Inventory { items: vec![cap, dagger, helm, bow, scarf, pebble] }, worn));
        stage.tick();
        stage.press(KeyCode::KeyI);
        (stage, bow, dagger)
    }

    /// Every line of the bag's words about the row picked out, under the
    /// rule, with the tone of each line's first letter.
    fn described(stage: &Stage) -> Vec<(String, ToneId)> {
        let rule = (1..stage.rows().len() as i32).find(|y| inside(stage, *y).starts_with('\u{2500}')).expect("a rule under the rows");
        let palette = stage.app.world().resource::<Palette>();
        let terminal = stage.app.world().resource::<Terminal>();
        let tone_of = |c| [Tones::TEXT, Tones::MUTED, Tones::GOOD, Tones::BAD].into_iter().find(|t| palette.get(*t) == c).unwrap_or(Tones::FRAME);
        (rule + 1..stage.rows().len() as i32 - 1)
            .map(|y| {
                let text = inside(stage, y);
                let lead = text.chars().take_while(|c| *c == ' ').count() as i32;
                let fg = terminal.get(2 + lead, y).map(|c| c.fg).unwrap_or_default();
                (text, tone_of(fg))
            })
            .filter(|(text, _)| !text.is_empty())
            .collect()
    }

    /// A helm picked out over a cap worn on the head says what it replaces
    /// and the armor each gives, the gain in the good tone, and a cap
    /// picked out over a helm the loss in the bad one.
    #[test]
    fn a_helm_picked_out_over_a_worn_cap_says_it_replaces_the_cap_and_the_armor_it_adds() {
        let (mut stage, _, _) = wardrobe();
        stage.press(KeyCode::ArrowDown);
        stage.press(KeyCode::ArrowDown);
        let lines = described(&stage);
        let texts: Vec<&str> = lines.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(texts, ["armor +2", "goes on the head", "replaces: cap", "  armor 1 -> 2 (+1)"]);
        assert_eq!(lines[3].1, Tones::GOOD, "more armor is better");
        assert!(!texts.iter().any(|t| t.contains("dagger")), "nothing about the hands");

        // The helm on, the cap is the worse of the two.
        let player = stage.player;
        let helm = stage.app.world().get::<Inventory>(player).unwrap().items[2];
        let head = stage.app.world().resource::<Registries>().slots.expect("head");
        stage.app.world_mut().get_mut::<Equipped>(player).unwrap().equip(helm, &EquipShape::in_slot(head)).unwrap();
        stage.press(KeyCode::ArrowUp);
        stage.press(KeyCode::ArrowUp);
        let lines = described(&stage);
        assert_eq!(lines.last().map(|(t, tone)| (t.as_str(), *tone)), Some(("  armor 2 -> 1 (-1)", Tones::BAD)), "{lines:?}");
    }

    /// A bow picked out says the right hand is bare and that it takes the
    /// left from the dagger, and what that gives up and gains: the blow
    /// struck goes from the dagger's to the wearer's own, which is the
    /// better here, and the bow's roll and reach are won.
    #[test]
    fn a_bow_over_a_dagger_says_the_hand_it_takes_and_the_blow_given_up() {
        let (mut stage, _, _) = wardrobe();
        stage.press(KeyCode::KeyD);
        stage.press(KeyCode::Escape);
        let lines = described(&stage);
        let texts: Vec<&str> = lines.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(
            texts,
            [
                "launches arrow 1d4 kinetic to 4",
                "goes on the right hand, and takes the left hand",
                "nothing worn on the right hand",
                "takes the left hand from the dagger",
                "  blow        dagger 1d4 -> own 1d6 (+1 avg)",
                "  arrow roll  none       -> 1d4     (+2.5 avg)",
                "  arrow range none       -> 4       (+4)",
            ],
            "the wearer strikes with its own blow once the dagger is off: {lines:?}"
        );
        let tones: Vec<ToneId> = lines[4..].iter().map(|(_, tone)| *tone).collect();
        assert_eq!(tones, [Tones::GOOD, Tones::GOOD, Tones::GOOD]);
    }

    /// A dagger picked out over the bow it would take off frees the hand
    /// the bow took and it does not.
    #[test]
    fn a_dagger_over_a_bow_frees_the_hand_it_does_not_take() {
        let (mut stage, bow, _) = wardrobe();
        let player = stage.player;
        let (right, left) = {
            let r = stage.app.world().resource::<Registries>();
            (r.slots.expect("right hand"), r.slots.expect("left hand"))
        };
        stage.app.world_mut().get_mut::<Equipped>(player).unwrap().equip(bow, &EquipShape::in_slot(right).and_claims(left)).unwrap();
        stage.press(KeyCode::ArrowDown);
        let texts: Vec<String> = described(&stage).into_iter().map(|(t, _)| t).collect();
        assert!(texts.contains(&"replaces: bow".to_string()) && texts.contains(&"frees the right hand".to_string()), "{texts:?}");
    }

    /// A wearer with nothing worn of a kind, a bag of the size asked, and
    /// the registries a comparison names: four slots, two things launched,
    /// a stat more is better of and one less is.
    fn closet(width: i32, height: i32) -> Stage {
        Stage::new_with(InventoryPanel::new(GridRect::new(0, 0, width, height)).title("Bag"), |app| {
            let mut registries = app.world_mut().resource_mut::<Registries>();
            registries.slots =
                Registry::from_defs(vec![SlotDef::new("right hand"), SlotDef::new("left hand"), SlotDef::new("head"), SlotDef::new("neck")]).unwrap();
            registries.tags = Registry::from_defs(vec![atglyph_rules::TagDef::new("arrow"), atglyph_rules::TagDef::new("stone")]).unwrap();
            registries.stats =
                Registry::from_defs(vec![atglyph_rules::StatDef::new("might", 0), atglyph_rules::StatDef::new("noise", 0).lower_is_better()]).unwrap();
        })
        .screen(width, height)
    }

    /// A slot of the closet's by name.
    fn slot(stage: &Stage, name: &str) -> SlotId {
        stage.app.world().resource::<Registries>().slots.expect(name)
    }

    /// The player carrying `bag` and wearing `worn`, the bag open on the
    /// row of `picked`.
    fn dress(stage: &mut Stage, bag: Vec<Entity>, worn: &[(Entity, EquipShape)], picked: Entity) {
        let mut equipped = Equipped(Equipment::with_slot_count(4));
        for (thing, shape) in worn {
            equipped.equip(*thing, shape).unwrap();
        }
        let at = bag.iter().position(|t| *t == picked).unwrap();
        let player = stage.player;
        stage.app.world_mut().entity_mut(player).insert((Inventory { items: bag }, equipped));
        stage.tick();
        stage.press(KeyCode::KeyI);
        stage.app.world_mut().resource_mut::<InventoryMenu>().selected = at;
        stage.tick();
    }

    /// The lines with the runs of spaces that set them in columns made one.
    fn squeezed(lines: &[(String, ToneId)]) -> Vec<(String, ToneId)> {
        let squeeze = |t: &str| " ".repeat(t.len() - t.trim_start().len()) + &t.split_whitespace().collect::<Vec<_>>().join(" ");
        lines.iter().map(|(t, tone)| (squeeze(t), *tone)).collect()
    }

    /// A knife for the off hand, the sword in the other striking first,
    /// changes no blow, and says nothing of one.
    #[test]
    fn an_unchanged_earlier_slot_keeps_the_blow_and_no_blow_is_compared() {
        let mut stage = closet(52, 22);
        let (right, left, kind) = (slot(&stage, "right hand"), slot(&stage, "left hand"), stage.kind);
        let w = stage.app.world_mut();
        let sword = w.spawn((Item, Name::new("sword"), Wearable(EquipShape::in_slot(right)), MeleeAttack::new(kind, DiceRoll::new(1, 8)))).id();
        let knife = w.spawn((Item, Name::new("knife"), Wearable(EquipShape::in_slot(left)), MeleeAttack::new(kind, DiceRoll::new(1, 4)))).id();
        dress(&mut stage, vec![sword, knife], &[(sword, EquipShape::in_slot(right))], knife);
        let texts: Vec<String> = described(&stage).into_iter().map(|(t, _)| t).collect();
        assert_eq!(texts, ["1d4 kinetic", "goes on the left hand", "nothing worn on the left hand"], "{texts:?}");
    }

    /// A versatile sword is weighed by the roll it would strike with on
    /// the wearer as it would then be dressed: its bigger roll while the
    /// left hand would be empty, and its own once a knife would fill it.
    #[test]
    fn a_versatile_blow_is_weighed_by_whether_the_left_hand_would_be_free() {
        use atglyph_rules::arms::Versatile;
        let blow_line = |stage: &Stage| squeezed(&described(stage)).into_iter().find(|(t, _)| t.trim_start().starts_with("blow "));
        let staged = |worn: &[&str], picked: &str| {
            let mut stage = closet(52, 22);
            let (right, left, kind) = (slot(&stage, "right hand"), slot(&stage, "left hand"), stage.kind);
            let blow = MeleeAttack::new(kind, DiceRoll::new(1, 8)).versatile(Versatile { roll: DiceRoll::new(1, 10), while_free: left });
            let w = stage.app.world_mut();
            let sword = w.spawn((Item, Name::new("sword"), Wearable(EquipShape::in_slot(right)), blow)).id();
            let knife = w.spawn((Item, Name::new("knife"), Wearable(EquipShape::in_slot(left)), MeleeAttack::new(kind, DiceRoll::new(1, 4)))).id();
            let by_name = |name: &str| if name == "sword" { (sword, EquipShape::in_slot(right)) } else { (knife, EquipShape::in_slot(left)) };
            let worn: Vec<(Entity, EquipShape)> = worn.iter().map(|n| by_name(n)).collect();
            dress(&mut stage, vec![sword, knife], &worn, by_name(picked).0);
            stage
        };
        assert_eq!(
            blow_line(&staged(&["sword"], "knife")),
            Some(("  blow sword 1d10 -> sword 1d8 (-1 avg)".to_string(), Tones::BAD)),
            "the knife would fill the hand the sword strikes harder for"
        );
        assert_eq!(
            blow_line(&staged(&[], "sword")),
            Some(("  blow own 1d6 -> sword 1d10 (+2 avg)".to_string(), Tones::GOOD)),
            "put on with the left hand empty, both hands"
        );
        assert_eq!(
            blow_line(&staged(&["knife"], "sword")),
            Some(("  blow knife 1d4 -> sword 1d8 (+2 avg)".to_string(), Tones::GOOD)),
            "put on beside the knife, one"
        );
    }

    /// Where a stuck thing holds the slot, the row says it will not come
    /// off, in the bad tone, and weighs no numbers, since it cannot go on.
    #[test]
    fn a_stuck_thing_in_the_way_is_named_in_the_bad_tone_and_nothing_is_weighed() {
        let mut stage = closet(52, 22);
        let head = slot(&stage, "head");
        let w = stage.app.world_mut();
        let cap = w.spawn((Item, Name::new("cap"), Wearable(EquipShape::in_slot(head)), Armor(1), atglyph_bevy::Stuck)).id();
        let helm = w.spawn((Item, Name::new("helm"), Wearable(EquipShape::in_slot(head)), Armor(2))).id();
        dress(&mut stage, vec![cap, helm], &[(cap, EquipShape::in_slot(head))], helm);
        let lines = described(&stage);
        assert_eq!(
            lines[2..],
            [("replaces: cap".to_string(), Tones::TEXT), ("the cap will not come off".to_string(), Tones::BAD)],
            "what stands in the way, and no armor: {lines:?}"
        );
    }

    /// A thing not yet known says where it goes and what it replaces, and
    /// weighs no numbers, the wearer's gear present or not.
    #[test]
    fn a_thing_not_yet_known_says_what_it_replaces_and_weighs_nothing() {
        let mut stage = closet(52, 22);
        let head = slot(&stage, "head");
        let w = stage.app.world_mut();
        let cap = w.spawn((Item, Name::new("cap"), Wearable(EquipShape::in_slot(head)), Armor(1))).id();
        let helm = w.spawn((Item, Name::new("helm"), Wearable(EquipShape::in_slot(head)), Armor(2), atglyph_bevy::Unknown)).id();
        dress(&mut stage, vec![cap, helm], &[(cap, EquipShape::in_slot(head))], helm);
        let texts: Vec<String> = described(&stage).into_iter().map(|(t, _)| t).collect();
        assert_eq!(texts, ["unknown", "goes on the head", "replaces: cap"], "{texts:?}");
    }

    /// Each number a thing lends is weighed against what is worn: its
    /// shot, extra roll, resistance, sight in the dark and stats, an
    /// average labelled as one, and a stat less is better of toned as a
    /// loss where it rises.
    #[test]
    fn a_thing_weighs_its_shot_extra_roll_resistance_sight_and_stats() {
        let mut stage = closet(72, 30);
        let (neck, kind) = (slot(&stage, "neck"), stage.kind);
        let (might, noise) = {
            let r = stage.app.world().resource::<Registries>();
            (r.stats.expect("might"), r.stats.expect("noise"))
        };
        let mut resists = atglyph_rules::Resistances::new();
        resists.set(kind, 10);
        let w = stage.app.world_mut();
        let charm = w
            .spawn((
                Item,
                Name::new("charm"),
                Wearable(EquipShape::in_slot(neck)),
                RangedAttack::new(kind, DiceRoll::new(1, 6), 5),
                BonusDamage(vec![(kind, DiceRoll::new(1, 4))]),
                Resists(resists),
                atglyph_bevy::WornDarkSight(3),
                Bestows(vec![(might, Op::Add(2)), (noise, Op::Add(1))]),
            ))
            .id();
        dress(&mut stage, vec![charm], &[], charm);
        let lines = squeezed(&described(&stage));
        for (line, tone) in [
            ("  shot none -> charm 1d6 to 5 (+3.5 avg)", Tones::GOOD),
            ("  extra none -> +1d4 kinetic (+2.5 avg)", Tones::GOOD),
            ("  resists kinetic 0% -> 10% (+10)", Tones::GOOD),
            ("  dark sight none -> 3 (+3)", Tones::GOOD),
            ("  might +0 -> +2 (+2)", Tones::GOOD),
            ("  noise +0 -> +1 (+1)", Tones::BAD),
        ] {
            assert!(lines.contains(&(line.to_string(), tone)), "{line:?} in {tone:?}: {lines:?}");
        }
    }

    /// A launcher put on in place of one for something else launched is
    /// weighed by what each launches, and neither is the better: the
    /// lines are in the plain tone.
    #[test]
    fn a_launcher_for_other_ammunition_is_weighed_by_its_ammunition_in_the_plain_tone() {
        let mut stage = closet(64, 24);
        let (right, left, kind) = (slot(&stage, "right hand"), slot(&stage, "left hand"), stage.kind);
        let (arrow, stone) = {
            let r = stage.app.world().resource::<Registries>();
            (r.tags.expect("arrow"), r.tags.expect("stone"))
        };
        let w = stage.app.world_mut();
        let sling = w
            .spawn((
                Item,
                Name::new("sling"),
                Wearable(EquipShape::in_slot(right)),
                atglyph_bevy::Launcher { ammo: stone, effective: 1, range: 2, strike: (kind, DiceRoll::new(1, 2)) },
            ))
            .id();
        let bow = w
            .spawn((
                Item,
                Name::new("bow"),
                Wearable(EquipShape::in_slot(right).and_claims(left)),
                atglyph_bevy::Launcher { ammo: arrow, effective: 2, range: 4, strike: (kind, DiceRoll::new(1, 4)) },
            ))
            .id();
        dress(&mut stage, vec![sling, bow], &[(sling, EquipShape::in_slot(right))], bow);
        let lines = squeezed(&described(&stage));
        let weighed: Vec<(String, ToneId)> = lines.into_iter().filter(|(t, _)| t.contains("->")).collect();
        assert_eq!(
            weighed,
            [
                ("  stone roll 1d2 -> none (-1.5 avg)".to_string(), Tones::TEXT),
                ("  stone range 2 -> none (-2)".to_string(), Tones::TEXT),
                ("  arrow roll none -> 1d4 (+2.5 avg)".to_string(), Tones::TEXT),
                ("  arrow range none -> 4 (+4)".to_string(), Tones::TEXT),
            ]
        );
    }

    /// Lines too wide for columns are said plainly and wrapped, each
    /// within the width, under the indent.
    #[test]
    fn changes_too_wide_for_columns_are_said_plainly_and_wrapped() {
        let lines = table(&[Change { what: "resists kinetic".into(), old: "0%".into(), new: "10%".into(), by: Some(20), unit: "", gain: Some(20) }], 18);
        assert!(lines.len() > 1 && lines.iter().all(|(t, _)| t.starts_with("  ") && t.chars().count() <= 18), "{lines:?}");
        assert_eq!(lines.iter().map(|(t, _)| t.trim()).collect::<Vec<_>>().join(" "), "resists kinetic 0% -> 10% (+10)");
    }

    /// Words past the bottom of the bag end its last row in a muted
    /// ellipsis at the right edge.
    #[test]
    fn words_past_the_bottom_of_the_bag_end_in_an_ellipsis() {
        let mut stage = closet(52, 10);
        let (neck, kind) = (slot(&stage, "neck"), stage.kind);
        let mut resists = atglyph_rules::Resistances::new();
        resists.set(kind, 10);
        let w = stage.app.world_mut();
        let charm = w.spawn((Item, Name::new("charm"), Wearable(EquipShape::in_slot(neck)), Armor(1), Resists(resists), atglyph_bevy::WornDarkSight(3))).id();
        dress(&mut stage, vec![charm], &[], charm);
        let terminal = stage.app.world().resource::<Terminal>();
        let palette = stage.app.world().resource::<Palette>();
        let last = terminal.get(52 - 3, 8).expect("the last row's end");
        assert_eq!((last.glyph, last.fg), ('\u{2026}', palette.get(Tones::MUTED)), "{}", stage.rows().join("\n"));
    }

    /// A thing put on for its like says it changes nothing, and one that
    /// differs only in what is not weighed says nothing at all.
    #[test]
    fn a_thing_over_its_like_says_nothing_changes() {
        let (mut stage, _, _) = wardrobe();
        let player = stage.player;
        let head = stage.app.world().resource::<Registries>().slots.expect("head");
        let spare = stage.app.world_mut().spawn((Item, Name::new("cap"), Wearable(EquipShape::in_slot(head)), Armor(1))).id();
        stage.app.world_mut().get_mut::<Inventory>(player).unwrap().items.push(spare);
        stage.tick();
        stage.press(KeyCode::KeyG);
        stage.press(KeyCode::Escape);
        let lines = described(&stage);
        assert_eq!(lines[2..], [("replaces: cap".to_string(), Tones::TEXT), ("  no change".to_string(), Tones::MUTED)], "{lines:?}");

        let thrown = stage.app.world_mut().spawn((Item, Name::new("cap"), Wearable(EquipShape::in_slot(head)), Armor(1), Throwable::new(Some(3), None))).id();
        stage.app.world_mut().get_mut::<Inventory>(player).unwrap().items.push(thrown);
        stage.tick();
        stage.press(KeyCode::ArrowDown);
        let lines = described(&stage);
        assert_eq!(
            lines.last().map(|(t, _)| t.as_str()),
            Some("replaces: cap"),
            "a cap that flies is not the same cap: {lines:?}\n{}",
            stage.rows().join("\n")
        );
    }

    /// A thing that goes where nothing is worn says so, and all it gives is
    /// a gain; a thing that is not worn at all, or is worn already,
    /// compares itself with nothing.
    #[test]
    fn a_bare_slot_says_nothing_is_worn_there_and_a_thing_not_worn_compares_with_nothing() {
        let (mut stage, _, _) = wardrobe();
        stage.press(KeyCode::KeyE);
        stage.press(KeyCode::Escape);
        let texts: Vec<String> = described(&stage).into_iter().map(|(t, _)| t).collect();
        assert_eq!(texts, ["armor +1", "goes on the neck", "nothing worn on the neck", "  armor 0 -> 1 (+1)"], "all it gives is a gain");

        stage.press(KeyCode::ArrowDown);
        let texts: Vec<String> = described(&stage).into_iter().map(|(t, _)| t).collect();
        assert!(texts.is_empty(), "a pebble says nothing at all: {texts:?}");
        stage.press(KeyCode::KeyA);
        stage.press(KeyCode::Escape);
        let texts: Vec<String> = described(&stage).into_iter().map(|(t, _)| t).collect();
        assert_eq!(texts, ["armor +1", "worn on the head"], "the cap worn replaces nothing");
    }

    /// The text inside the frame on row `y`, border and padding cut off.
    fn inside(stage: &Stage, y: i32) -> String {
        stage.row(y).trim_start_matches("\u{2502} ").trim_end_matches('\u{2502}').trim_end().to_string()
    }

    /// A bag with more rows than its list has room for says how many are
    /// out of view above, on the line over the list, and below, on the rule
    /// under it, in the muted tone; and the list moves only as far as keeps
    /// the row picked out in view, so it holds still under a cursor moving
    /// inside it.
    #[test]
    fn a_bag_taller_than_its_rectangle_marks_the_rows_out_of_view_and_keeps_the_picked_row_in_it() {
        // Eight rows inside the frame: four to the list, the rule on the
        // fifth row from the top.
        let mut stage = Stage::new(InventoryPanel::new(GridRect::new(0, 0, 40, 10)).title("Bag")).screen(40, 10);
        let player = stage.player;
        let pebbles: Vec<Entity> = (0..10).map(|i| stage.app.world_mut().spawn((Item, Name::new(format!("pebble {i}")))).id()).collect();
        stage.app.world_mut().get_mut::<Inventory>(player).unwrap().items = pebbles;
        stage.tick();
        stage.press(KeyCode::KeyI);
        let listed = |stage: &Stage| (1..5).map(|y| inside(stage, y)).collect::<Vec<_>>();
        assert_eq!(listed(&stage), ["a) pebble 0", "b) pebble 1", "c) pebble 2", "d) pebble 3"]);
        assert!(!stage.row(0).contains('\u{25b2}'), "nothing above the first row: {:?}", stage.row(0));
        assert!(stage.row(5).contains(" \u{25bc} 6 more "), "six below, on the rule: {:?}", stage.row(5));

        for _ in 0..4 {
            stage.press(KeyCode::ArrowDown);
        }
        assert_eq!(listed(&stage), ["b) pebble 1", "c) pebble 2", "d) pebble 3", "e) pebble 4"], "down past the foot moves the list by one");
        assert!(stage.row(0).contains(" \u{25b2} 1 more ") && stage.row(0).contains(" Bag "), "one above, beside the title: {:?}", stage.row(0));
        assert!(stage.row(5).contains(" \u{25bc} 5 more "), "{:?}", stage.row(5));
        stage.press(KeyCode::ArrowUp);
        assert_eq!(listed(&stage), ["b) pebble 1", "c) pebble 2", "d) pebble 3", "e) pebble 4"], "back up inside the list, it holds still");

        // A game that picks a row itself has it brought into view.
        stage.app.world_mut().resource_mut::<InventoryMenu>().selected = 9;
        stage.tick();
        assert_eq!(listed(&stage), ["g) pebble 6", "h) pebble 7", "i) pebble 8", "j) pebble 9"]);
        assert!(stage.row(0).contains(" \u{25b2} 6 more ") && !stage.row(5).contains('\u{25bc}'), "{:?} {:?}", stage.row(0), stage.row(5));
        let terminal = stage.app.world().resource::<Terminal>();
        let x = stage.row(0).chars().position(|c| c == '\u{25b2}').unwrap() as i32;
        assert_eq!(terminal.get(x, 0).map(|c| c.fg), Some(stage.app.world().resource::<Palette>().get(Tones::MUTED)), "in the muted tone");
    }

    #[test]
    fn the_bag_opens_lists_what_is_carried_and_describes_the_row_picked_out() {
        let (mut stage, _, _, _) = staged();
        assert!(stage.rows().iter().all(|r| r.is_empty()), "nothing until the key");
        stage.press(KeyCode::KeyI);
        assert!(stage.row(0).contains(" Bag "), "{:?}", stage.row(0));
        let first = inside(&stage, 1);
        assert!(first.starts_with("a) ) a blade") && first.ends_with("hand"), "letter, glyph, name, and the slot it is worn in at the right: {first:?}");
        assert_eq!(inside(&stage, 2), "b) a hat");
        assert_eq!(inside(&stage, 3), "c) 3 knives", "a stack says how many");
        assert!(stage.row(13).contains("a-c enter open \u{2022} esc close"), "the list offers its letters: {:?}", stage.row(13));
        assert_eq!(inside(&stage, 5), "1d6 kinetic", "the blade's own blow");
        assert_eq!(inside(&stage, 6), "worn on the hand");
        stage.press(KeyCode::Enter);
        assert!(stage.row(13).contains("e take off \u{2022} d drop \u{2022} esc back"), "the worn blade can come off or be dropped: {:?}", stage.row(13));
        stage.press(KeyCode::Escape);

        stage.press(KeyCode::ArrowDown);
        assert_eq!(inside(&stage, 5), "armor +1");
        assert_eq!(inside(&stage, 6), "resists kinetic 10%", "what it resists, under its armor");
        assert_eq!(inside(&stage, 7), "goes on the head");
        stage.press(KeyCode::Enter);
        assert!(stage.row(13).contains("e wear \u{2022} d drop \u{2022} esc back"), "the hat goes on: {:?}", stage.row(13));
        stage.press(KeyCode::Escape);
        stage.press(KeyCode::ArrowDown);
        assert_eq!(inside(&stage, 5), "thrown 1d4 kinetic to 5");
        stage.press(KeyCode::Enter);
        assert!(
            stage.row(13).contains("d drop \u{2022} t throw \u{2022} esc back") && !stage.row(13).contains("wear"),
            "knives are thrown, never worn: {:?}",
            stage.row(13)
        );

        stage.press(KeyCode::Escape);
        stage.press(KeyCode::Escape);
        assert!(!stage.app.world().resource::<Modals>().any_open());
    }

    /// Where the cursor is, and whether its row is opened.
    fn menu(stage: &Stage) -> (usize, bool) {
        let menu = stage.app.world().resource::<InventoryMenu>();
        (menu.selected, menu.opened)
    }

    /// Whether the bag is up at all.
    fn up(stage: &Stage) -> bool {
        let modals = stage.app.world().resource::<Modals>();
        modals.is_open(inventory_modal(modals))
    }

    /// Every intent the bag wrote since the last call, as a count.
    fn intents_written(stage: &mut Stage) -> usize {
        let w = stage.app.world_mut();
        w.resource_mut::<Messages<Intent<Unequip>>>().drain().count()
            + w.resource_mut::<Messages<Intent<Equip>>>().drain().count()
            + w.resource_mut::<Messages<Intent<DropItem>>>().drain().count()
            + w.resource_mut::<Messages<Intent<UseItem>>>().drain().count()
            + w.resource_mut::<Messages<AimThrow>>().drain().count()
    }

    /// A player carrying `n` plain things named by their place, in a bag
    /// frame of the usual size.
    fn carrying(n: usize) -> Stage {
        let mut stage = Stage::new(InventoryPanel::new(GridRect::new(0, 0, 52, 14)).title("Bag")).screen(52, 14);
        let items = (0..n).map(|i| stage.app.world_mut().spawn((Item, Name::new(format!("thing {i}")))).id()).collect();
        let player = stage.player;
        stage.app.world_mut().entity_mut(player).insert(Inventory { items });
        stage.tick();
        stage
    }

    /// The letters run `a` to `z` down the list, by a row's place in it and
    /// not its place on screen, and a row past `z` has none but keeps its
    /// name in line with the rest.
    #[test]
    fn rows_are_lettered_a_to_z_in_list_order_and_rows_past_z_have_none() {
        let mut stage = carrying(28);
        stage.press(KeyCode::KeyI);
        assert_eq!([inside(&stage, 1), inside(&stage, 2)], ["a) thing 0", "b) thing 1"]);
        stage.press(KeyCode::ArrowUp);
        assert_eq!(menu(&stage), (27, false), "up from the top wraps to the last row");
        let shown: Vec<String> = (1..7).map(|y| inside(&stage, y)).collect();
        assert_eq!(shown[3..], ["z) thing 25", "   thing 26", "   thing 27"], "{shown:?}");
        assert!(stage.row(13).contains("a-z enter open"), "{:?}", stage.row(13));
        stage.press(KeyCode::KeyZ);
        assert_eq!(menu(&stage), (25, true), "z is the twenty-sixth row, never one past it");
    }

    /// On the list a letter picks out its row and opens it in one press,
    /// and writes nothing; a letter with no row does nothing.
    #[test]
    fn a_letter_on_the_list_picks_out_its_row_and_opens_it() {
        let (mut stage, _, _, _) = staged();
        stage.press(KeyCode::KeyI);
        stage.press(KeyCode::KeyZ);
        assert_eq!(menu(&stage), (0, false), "no row z, so nothing");
        stage.press(KeyCode::KeyC);
        assert_eq!(menu(&stage), (2, true), "c is the knives, opened");
        assert!(stage.row(13).contains("d drop \u{2022} t throw \u{2022} esc back"), "{:?}", stage.row(13));
        assert_eq!(intents_written(&mut stage), 0, "opening a row spends nothing");
        stage.press(KeyCode::KeyA);
        assert_eq!(menu(&stage), (2, true), "and an opened row reads no letters");
    }

    /// Enter on the list opens the row under the cursor, the way to a row
    /// past `z`, rather than using it; on an opened row the direction keys
    /// do not walk away from it.
    #[test]
    fn enter_on_the_list_opens_the_row_under_the_cursor_and_uses_nothing() {
        let (mut stage, triggers) = with_triggers(r#"[(on: "use", effects: [(kind: "Mend", args: (roll: "5"))])]"#);
        let player = stage.player;
        let salve = stage.app.world_mut().spawn((Item, Name::new("salve"), triggers.clone(), Consumable::new(1, WhenEmpty::Destroyed))).id();
        let tonic = stage.app.world_mut().spawn((Item, Name::new("tonic"), triggers, Consumable::new(1, WhenEmpty::Destroyed))).id();
        stage.app.world_mut().entity_mut(player).insert(Inventory { items: vec![salve, tonic] });
        stage.tick();

        stage.press(KeyCode::KeyI);
        stage.press(KeyCode::ArrowDown);
        stage.press(KeyCode::Enter);
        assert_eq!(menu(&stage), (1, true), "the tonic, opened");
        assert_eq!(intents_written(&mut stage), 0, "and not drunk");
        stage.press(KeyCode::ArrowUp);
        assert_eq!(menu(&stage), (1, true), "an opened row stays opened on its own row");
        stage.press(KeyCode::Enter);
        let uses: Vec<Entity> = stage.app.world_mut().resource_mut::<Messages<Intent<UseItem>>>().drain().map(|i| i.action.0).collect();
        assert_eq!(uses, vec![tonic], "Enter on the opened row uses it");
    }

    /// The action keys are letters on the list, and act only on an opened
    /// row: `e` on a list of five opens the fifth row and wears nothing.
    #[test]
    fn the_action_keys_act_only_on_an_opened_row() {
        let (mut stage, blade, _, _) = staged();
        let player = stage.player;
        let more: Vec<Entity> = (0..2).map(|i| stage.app.world_mut().spawn((Item, Name::new(format!("pebble {i}")))).id()).collect();
        stage.app.world_mut().get_mut::<Inventory>(player).unwrap().items.extend(more);
        stage.tick();

        stage.press(KeyCode::KeyI);
        stage.press(KeyCode::KeyE);
        assert_eq!(menu(&stage), (4, true), "e is the fifth row, opened");
        assert_eq!(intents_written(&mut stage), 0, "and nothing is worn or taken off");
        stage.press(KeyCode::Escape);
        stage.press(KeyCode::KeyA);
        stage.press(KeyCode::KeyE);
        let off: Vec<Entity> = stage.app.world_mut().resource_mut::<Messages<Intent<Unequip>>>().drain().map(|i| i.action.0).collect();
        assert_eq!(off, vec![blade], "e on the opened blade takes it off");
        assert!(!up(&stage), "and the bag is put away for the turn");
    }

    /// Esc steps back one step at a time: from an opened row to the list,
    /// and from the list out of the bag. The toggle's own letter is a
    /// row's on the list and the toggle again on an opened row, and the bag
    /// always comes back up on the list.
    #[test]
    fn esc_goes_back_from_an_opened_row_to_the_list_and_from_the_list_out() {
        let (mut stage, _, _, _) = staged();
        stage.press(KeyCode::KeyI);
        stage.press(KeyCode::KeyB);
        stage.press(KeyCode::Escape);
        assert_eq!((menu(&stage), up(&stage)), ((1, false), true), "back on the list, on the same row");
        stage.press(KeyCode::KeyI);
        assert_eq!((menu(&stage), up(&stage)), ((1, false), true), "i on the list is row i's, and there is none");
        stage.press(KeyCode::Escape);
        assert!(!up(&stage), "and out");

        stage.press(KeyCode::KeyI);
        stage.press(KeyCode::KeyB);
        stage.press(KeyCode::KeyI);
        assert!(!up(&stage), "on an opened row the toggle closes the bag");
        stage.press(KeyCode::KeyI);
        assert_eq!(menu(&stage), (1, false), "which comes back up on the list");
    }

    /// Ten rows, frames a tenth of a second apart, and the repeat's pace
    /// set to fire on every frame once it starts.
    fn timed(rows: usize) -> Stage {
        let mut stage = carrying(rows);
        stage.app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_millis(100)));
        stage.app.insert_resource(crate::RepeatPace { delay: 0.25, every: 0.05 });
        stage.press(KeyCode::KeyI);
        stage
    }

    /// The row the cursor is on after each of `frames` frames with `key`
    /// held down from the first.
    fn holding(stage: &mut Stage, key: KeyCode, frames: usize) -> Vec<usize> {
        stage.app.world_mut().resource_mut::<atglyph_bevy::testing::KeyScript>().hold(key);
        let walked = (0..frames)
            .map(|_| {
                stage.tick();
                menu(stage).0
            })
            .collect();
        stage.app.world_mut().resource_mut::<atglyph_bevy::testing::KeyScript>().release(key);
        stage.tick();
        walked
    }

    /// A direction key held down walks the list at the pace the look
    /// cursor steps at: the press, then nothing until the delay is up, then
    /// a row each time the repeat fires.
    #[test]
    fn a_held_direction_key_walks_the_list_at_the_cursors_pace() {
        let mut stage = timed(10);
        assert_eq!(holding(&mut stage, KeyCode::ArrowDown, 6), vec![1, 1, 1, 2, 3, 4]);
        assert_eq!(menu(&stage), (4, false), "and stops when let go");
    }

    /// A direction bound to a letter is the letter's on the list, held or
    /// pressed: `j` is the tenth row, never a step down a shorter list.
    #[test]
    fn a_held_letter_never_walks_the_list_where_it_is_also_a_direction() {
        let mut stage = timed(5);
        assert_eq!(holding(&mut stage, KeyCode::KeyJ, 6), vec![0; 6]);
        assert_eq!(menu(&stage), (0, false));
    }

    /// A game's facet reads with the thing's own numbers, since it is a
    /// fact about the thing the engine could not know, and the muted line
    /// saying where it goes stays under everything the thing is, as it
    /// does for a thing with no facet.
    #[test]
    fn a_games_facet_reads_under_the_things_numbers_and_above_where_it_goes() {
        let (mut stage, _, hat, _) = staged();
        stage.app.add_systems(
            Update,
            (move |mut view: ResMut<InventoryView>| {
                for row in view.rows.iter_mut().filter(|r| r.entity == hat) {
                    row.facets.push(crate::Facet::new("dark sight", "sees 8 in the dark"));
                }
            })
            .in_set(crate::ViewSet::Annotate),
        );
        stage.tick();
        stage.press(KeyCode::KeyI);
        stage.press(KeyCode::ArrowDown);
        assert_eq!(detail(&stage)[4..8], ["armor +1", "resists kinetic 10%", "sees 8 in the dark", "goes on the head"], "{:?}", detail(&stage));
    }

    /// Each key writes the engine's own intent for the row picked out and
    /// puts every screen away, since the action spends the turn.
    #[test]
    fn the_drop_prompt_is_drawn_over_the_pack() {
        let (mut stage, _, _, knives) = staged();
        stage.press(KeyCode::KeyI);
        let row = stage.app.world().resource::<InventoryView>().rows.iter().position(|r| r.entity == knives).expect("the knives are listed");
        stage.press([KeyCode::KeyA, KeyCode::KeyB, KeyCode::KeyC][row]);
        stage.press(KeyCode::KeyD);
        stage.tick();
        assert!(stage.rows().iter().any(|line| line.contains("Drop how many knives?")), "{:#?}", stage.rows());
    }

    #[test]
    fn the_packs_drop_asks_how_many_of_a_stack() {
        let (mut stage, _, _, knives) = staged();
        stage.press(KeyCode::KeyI);
        let row = stage.app.world().resource::<InventoryView>().rows.iter().position(|r| r.entity == knives).expect("the knives are listed");
        stage.press([KeyCode::KeyA, KeyCode::KeyB, KeyCode::KeyC][row]);
        stage.press(KeyCode::KeyD);
        let counting = stage.app.world().resource::<crate::panel::ground::Counting>();
        assert_eq!((counting.count, counting.most), (3, 3), "a stack of three asks how many, starting at all");
        stage.press(KeyCode::Digit2);
        stage.press(KeyCode::Enter);
        let drops: Vec<DropItem> = stage.app.world_mut().resource_mut::<Messages<Intent<DropItem>>>().drain().map(|i| i.action).collect();
        assert_eq!(drops, vec![DropItem { item: knives, count: Some(2) }]);
        let modals = stage.app.world().resource::<Modals>();
        assert!(!modals.is_open(inventory_modal(modals)), "the drop closed the bag");
    }

    #[test]
    fn the_keys_wear_drop_use_and_throw_the_row_picked_out() {
        let (mut stage, blade, hat, knives) = staged();
        let player = stage.player;
        let drain = |stage: &mut Stage| {
            let w = stage.app.world_mut();
            let unequips: Vec<Entity> = w.resource_mut::<Messages<Intent<Unequip>>>().drain().map(|i| i.action.0).collect();
            let equips: Vec<Entity> = w.resource_mut::<Messages<Intent<Equip>>>().drain().map(|i| i.action.0).collect();
            let drops: Vec<Entity> = w.resource_mut::<Messages<Intent<DropItem>>>().drain().map(|i| i.action.item).collect();
            let uses: Vec<Entity> = w.resource_mut::<Messages<Intent<UseItem>>>().drain().map(|i| i.action.0).collect();
            let throws: Vec<Entity> = w.resource_mut::<Messages<AimThrow>>().drain().map(|a| a.item).collect();
            (unequips, equips, drops, uses, throws)
        };
        let closed = |stage: &Stage| !stage.app.world().resource::<Modals>().is_open(inventory_modal(stage.app.world().resource::<Modals>()));

        stage.press(KeyCode::KeyI);
        stage.press(KeyCode::KeyA);
        stage.press(KeyCode::KeyE);
        assert_eq!(drain(&mut stage).0, vec![blade], "the worn blade is taken off");
        assert!(closed(&stage));
        stage.tick();
        assert!(stage.app.world().get::<Equipped>(player).unwrap().slot_of(blade).is_none(), "and the engine resolved it");

        stage.press(KeyCode::KeyI);
        stage.press(KeyCode::KeyB);
        stage.press(KeyCode::KeyE);
        assert_eq!(drain(&mut stage).1, vec![hat], "the hat goes on");
        assert!(closed(&stage));

        stage.press(KeyCode::KeyI);
        stage.press(KeyCode::Enter);
        stage.press(KeyCode::KeyD);
        assert_eq!(drain(&mut stage).2, vec![hat], "the cursor stayed on the hat, which is dropped");
        stage.tick();
        stage.press(KeyCode::KeyI);
        assert_eq!(inside(&stage, 2), "b) 3 knives", "and is out of the bag");
        stage.press(KeyCode::KeyB);
        stage.press(KeyCode::Enter);
        assert!(drain(&mut stage).3.is_empty(), "knives lend nothing to use, so confirm does nothing and spends no turn");
        assert!(!closed(&stage), "and the bag stays up");

        stage.press(KeyCode::Escape);
        stage.press(KeyCode::KeyA);
        stage.press(KeyCode::KeyT);
        assert!(drain(&mut stage).4.is_empty(), "a blade is not thrown");
        assert!(!closed(&stage), "so the bag stays up");
        stage.press(KeyCode::Escape);
        stage.press(KeyCode::KeyB);
        stage.press(KeyCode::KeyT);
        assert_eq!(drain(&mut stage).4, vec![knives], "the knives are");
        assert!(closed(&stage), "and the bag closed for the cursor");
    }

    /// A game that opens the bag on its own key, one the bag also reads,
    /// opens it and no more: the press that put the screen up is not read
    /// again inside it, whichever of the two systems runs first. Here it
    /// opens the bag on a row, the way a throw key opens it on what flies.
    #[test]
    fn a_game_key_that_opens_the_bag_is_not_also_read_by_it() {
        let (mut stage, blade, hat, knives) = staged();
        let player = stage.player;
        stage.app.world_mut().entity_mut(player).insert(Inventory { items: vec![knives, blade, hat] });
        stage.app.add_systems(
            Update,
            (|keys: Res<ButtonInput<KeyCode>>, mut modals: ResMut<Modals>, mut menu: ResMut<InventoryMenu>| {
                if keys.just_pressed(KeyCode::KeyT) && !modals.any_open() {
                    let bag = inventory_modal(&modals);
                    menu.open(0);
                    modals.open(bag);
                }
            })
            .in_set(atglyph_bevy::EngineSet::Input)
            .before(inventory_keys),
        );
        stage.tick();

        stage.press(KeyCode::KeyT);
        let modals = stage.app.world().resource::<Modals>();
        assert!(modals.is_top(inventory_modal(modals)), "the bag is up");
        assert!(stage.app.world_mut().resource_mut::<Messages<AimThrow>>().drain().next().is_none(), "and the knives on its first row are not thrown yet");

        stage.press(KeyCode::KeyT);
        let thrown: Vec<Entity> = stage.app.world_mut().resource_mut::<Messages<AimThrow>>().drain().map(|a| a.item).collect();
        assert_eq!(thrown, vec![knives], "the second press is the bag's");
    }

    #[test]
    fn an_empty_bag_says_so_in_the_games_words() {
        let (mut stage, _, _, _) = staged();
        let player = stage.player;
        stage.app.world_mut().entity_mut(player).insert(Inventory::default());
        stage.press(KeyCode::KeyI);
        assert_eq!(inside(&stage, 1), "Lint.");
    }

    /// A bag whose items carry `triggers`, built the way a game's loader
    /// builds them, through the registered moments and effect kinds.
    fn with_triggers(triggers: &str) -> (Stage, atglyph_bevy::Triggers) {
        let stage = Stage::new_with((atglyph_bevy::EffectsPlugin, InventoryPanel::new(GridRect::new(0, 0, 52, 14)).title("Bag")), |app| {
            app.add_engine_effects();
        })
        .screen(52, 14);
        let specs: Vec<atglyph_rules::TriggerSpec> = Names::new().load_list(triggers).expect("the triggers parse");
        let built = {
            let world = stage.app.world();
            let registries = world.resource::<Registries>().clone();
            atglyph_bevy::Triggers::build(&specs, &[], world.resource::<atglyph_bevy::TriggerKinds>(), world.resource::<EffectKinds>(), &registries.names())
                .expect("the triggers build")
        };
        (stage, built)
    }

    /// Every line of the bag's detail pane, top to bottom.
    fn detail(stage: &Stage) -> Vec<String> {
        (1..13).map(|y| inside(stage, y)).collect()
    }

    /// A thing with a `use` trigger is offered and described from its own
    /// effects, with no ability anywhere, and the use key uses it.
    #[test]
    fn a_thing_with_a_use_trigger_is_offered_described_and_used() {
        let (mut stage, triggers) = with_triggers(r#"[(on: "use", effects: [(kind: "Mend", args: (roll: "5"))])]"#);
        let player = stage.player;
        let poultice =
            stage.app.world_mut().spawn((Item, Name::new("poultice"), triggers, Consumable::new(1, WhenEmpty::Destroyed), Stack { key: 1, count: 2 })).id();
        stage.app.world_mut().entity_mut(player).insert(Inventory { items: vec![poultice] });
        stage.tick();

        stage.press(KeyCode::KeyI);
        assert_eq!(inside(&stage, 1), "a) 2 poultices");
        assert!(detail(&stage).iter().any(|l| l == "use: mends 5"), "described by what it does: {:?}", detail(&stage));
        assert!(stage.app.world().resource::<InventoryView>().rows[0].usable(), "and the row knows it is used");

        stage.press(KeyCode::KeyA);
        stage.press(KeyCode::KeyU);
        let uses: Vec<Entity> = stage.app.world_mut().resource_mut::<Messages<Intent<UseItem>>>().drain().map(|i| i.action.0).collect();
        assert_eq!(uses, vec![poultice], "and the use key uses it");
    }

    /// A grenade says what it does where it lands, and is thrown rather than
    /// used: the use key does nothing and the footer does not offer it.
    #[test]
    fn a_thrown_thing_reads_what_it_does_where_it_lands_and_offers_no_use() {
        let (mut stage, triggers) = with_triggers(r#"[(on: "land", area: Burst(radius: 1), effects: [(kind: "Harm", args: (kind: "kinetic", roll: "3"))])]"#);
        let player = stage.player;
        let grenade =
            stage.app.world_mut().spawn((Item, Name::new("grenade"), triggers, Consumable::new(1, WhenEmpty::Destroyed), Throwable::new(Some(6), None))).id();
        stage.app.world_mut().entity_mut(player).insert(Inventory { items: vec![grenade] });
        stage.tick();

        stage.press(KeyCode::KeyI);
        assert!(detail(&stage).iter().any(|l| l == "on landing: 3 kinetic in a burst of 1"), "{:?}", detail(&stage));
        assert!(!stage.app.world().resource::<InventoryView>().rows[0].usable(), "nothing to use");
        stage.press(KeyCode::KeyA);
        stage.press(KeyCode::KeyU);
        assert!(stage.app.world_mut().resource_mut::<Messages<Intent<UseItem>>>().drain().next().is_none(), "and the use key spends no turn on it");
    }

    /// A wand counts its charges on its row, and an empty one is not offered
    /// a use it would be refused.
    #[test]
    fn a_wand_counts_its_charges_and_an_empty_one_offers_no_use() {
        let (mut stage, triggers) = with_triggers(r#"[(on: "use", effects: [(kind: "Mend", args: (roll: "2"))])]"#);
        let player = stage.player;
        let wand = stage.app.world_mut().spawn((Item, Name::new("wand"), triggers.clone(), Consumable { left: 3, ..Consumable::new(5, WhenEmpty::Kept) })).id();
        let spent = stage.app.world_mut().spawn((Item, Name::new("dead wand"), triggers, Consumable { left: 0, ..Consumable::new(5, WhenEmpty::Kept) })).id();
        stage.app.world_mut().entity_mut(player).insert(Inventory { items: vec![wand, spent] });
        stage.tick();

        stage.press(KeyCode::KeyI);
        assert!(detail(&stage).iter().any(|l| l == "3/5 charges"), "{:?}", detail(&stage));
        stage.press(KeyCode::ArrowDown);
        assert!(detail(&stage).iter().any(|l| l == "empty"), "{:?}", detail(&stage));
        let rows = &stage.app.world().resource::<InventoryView>().rows;
        assert!(rows[0].usable() && !rows[1].usable(), "a live wand is used and a dead one is not");
        stage.press(KeyCode::KeyB);
        stage.press(KeyCode::KeyU);
        assert!(stage.app.world_mut().resource_mut::<Messages<Intent<UseItem>>>().drain().next().is_none(), "so the use key does nothing on it");
    }

    /// Two attuned plates, one worn and a quarter charged and one spare in
    /// the bag: the worn one says when it will be ready, and the spare says
    /// why it never will be while it stays there, and that putting it on
    /// starts it from nothing.
    #[test]
    fn a_charging_plate_says_when_it_is_ready_and_a_spare_says_it_charges_only_worn() {
        let (mut stage, triggers) = with_triggers(r#"[(on: "use", effects: [(kind: "Mend", args: (roll: "2"))])]"#);
        stage.app.world_mut().resource_mut::<Registries>().slots = Registry::from_defs(vec![SlotDef::new("torso")]).unwrap();
        let player = stage.player;
        let torso = stage.app.world().resource::<Registries>().slots.expect("torso");
        let charging = Consumable { left: 0, recharge: Some(atglyph_bevy::Recharge { every: 4000, progress: 1000 }), ..Consumable::new(1, WhenEmpty::Kept) };
        let plate = |name: &str| (Item, Name::new(name.to_string()), triggers.clone(), charging, atglyph_bevy::Attuned, Wearable(EquipShape::in_slot(torso)));
        let worn_plate = stage.app.world_mut().spawn(plate("plate")).id();
        let spare = stage.app.world_mut().spawn(plate("spare")).id();
        let charged = stage.app.world_mut().spawn(plate("charged")).id();
        stage.app.world_mut().get_mut::<Consumable>(charged).unwrap().left = 1;
        let mut worn = Equipped(Equipment::with_slot_count(1));
        worn.equip(worn_plate, &EquipShape::in_slot(torso)).unwrap();
        stage.app.world_mut().entity_mut(player).insert((Inventory { items: vec![worn_plate, spare, charged] }, worn));
        stage.tick();

        stage.press(KeyCode::KeyI);
        assert!(detail(&stage).iter().any(|l| l == "ready in 30 turns"), "{:?}", detail(&stage));
        stage.press(KeyCode::ArrowDown);
        assert!(detail(&stage).iter().any(|l| l == "empties when put on; charges only while worn"), "{:?}", detail(&stage));
        assert!(!detail(&stage).iter().any(|l| l == "empty"), "the reason, not the bare fact: {:?}", detail(&stage));
        stage.press(KeyCode::ArrowDown);
        assert!(
            detail(&stage).iter().any(|l| l == "empties when put on; charges only while worn"),
            "a charged one taken off loses its charge going back on, and says so: {:?}",
            detail(&stage)
        );
    }

    /// A thing used at somewhere says how it is aimed, in the words the
    /// ability list uses, and its use key asks for the targeting cursor
    /// rather than using it where the player stands.
    #[test]
    fn a_thing_used_at_somewhere_says_how_it_is_aimed_and_its_use_key_asks_for_the_cursor() {
        let (mut stage, triggers) =
            with_triggers(r#"[(on: "use", targeting: (mode: Bolt(range: 6)), effects: [(kind: "Harm", args: (kind: "kinetic", roll: "4"))])]"#);
        let player = stage.player;
        let rod = stage.app.world_mut().spawn((Item, Name::new("rod"), triggers, Consumable::new(3, WhenEmpty::Kept))).id();
        stage.app.world_mut().entity_mut(player).insert(Inventory { items: vec![rod] });
        stage.tick();

        stage.press(KeyCode::KeyI);
        assert!(detail(&stage).iter().any(|l| l == "aimed at a foe, a bolt up to 6 away"), "{:?}", detail(&stage));
        stage.press(KeyCode::KeyA);
        stage.press(KeyCode::KeyU);
        let aims: Vec<AimUse> = stage.app.world_mut().resource_mut::<Messages<AimUse>>().drain().collect();
        assert_eq!(aims, vec![AimUse { user: player, item: rod }], "the cursor is asked for");
        assert!(stage.app.world_mut().resource_mut::<Messages<Intent<UseItem>>>().drain().next().is_none(), "and nothing is used yet");
        assert!(!stage.app.world().resource::<Modals>().any_open(), "the bag closed for the cursor");
    }

    /// The whole of it from the bag: a rod aimed and put away spends no
    /// charge and passes no turn, and aimed again and confirmed it lands
    /// its bolt on the foe it was pointed at and not on whoever used it.
    #[test]
    fn a_use_aimed_from_the_bag_and_put_away_spends_nothing_and_confirmed_lands_where_it_was_pointed() {
        let mut stage = Stage::new_with((atglyph_bevy::EffectsPlugin, crate::TargetViewPlugin, InventoryPanel::new(GridRect::new(0, 0, 52, 14))), |app| {
            app.add_engine_effects();
        })
        .screen(52, 14);
        let specs: Vec<atglyph_rules::TriggerSpec> = Names::new()
            .load_list(r#"[(on: "use", targeting: (mode: Bolt(range: 6)), effects: [(kind: "Harm", args: (kind: "kinetic", roll: "4"))])]"#)
            .expect("the trigger parses");
        let triggers = {
            let world = stage.app.world();
            let names = world.resource::<Registries>().names();
            atglyph_bevy::Triggers::build(&specs, &[], world.resource::<atglyph_bevy::TriggerKinds>(), world.resource::<EffectKinds>(), &names)
                .expect("it builds")
        };
        let player = stage.player;
        let rod = stage.app.world_mut().spawn((Item, Name::new("rod"), triggers, Consumable::new(3, WhenEmpty::Kept))).id();
        stage.app.world_mut().entity_mut(player).insert(Inventory { items: vec![rod] });
        let foe = stage.actor("rat", 'r', 3, 0);
        stage.tick();
        let hp = |stage: &Stage, who: Entity| stage.app.world().get::<Health>(who).expect("health").current;
        let charges = |stage: &Stage| stage.app.world().get::<Consumable>(rod).expect("charges").left;
        let now = |stage: &Stage| stage.app.world().resource::<Turns>().now();
        let before = now(&stage);

        stage.press(KeyCode::KeyI);
        stage.press(KeyCode::KeyA);
        stage.press(KeyCode::KeyU);
        let view = stage.app.world().resource::<crate::TargetView>();
        assert_eq!((view.using, view.cursor), (Some(rod), stage.at.offset(3, 0)), "the cursor opened on the rat");
        stage.press(KeyCode::Escape);
        assert!(!stage.app.world().resource::<crate::TargetView>().aiming() && !stage.app.world().resource::<Modals>().any_open(), "put away");
        assert_eq!((charges(&stage), now(&stage), hp(&stage, foe)), (3, before, 10), "no charge, no time, no harm");
        assert!(stage.app.world().get::<MyTurn>(player).is_some(), "and the turn is still the player's");

        stage.press(KeyCode::KeyI);
        stage.press(KeyCode::KeyA);
        stage.press(KeyCode::KeyU);
        stage.press(KeyCode::Enter);
        assert_eq!((hp(&stage, foe), hp(&stage, player)), (6, 30), "the bolt landed on the rat, not on its user");
        assert_eq!(charges(&stage), 2, "for a charge");
        assert!(now(&stage) > before, "and the turn");
    }

    /// What a game's own effect leaves on the thing a use was made on.
    #[derive(Component)]
    struct Marked;

    /// A game's own effect for a use made on a thing: it marks the thing.
    struct Mark;

    impl atglyph_bevy::Effect for Mark {
        fn apply(&self, landing: &atglyph_bevy::Landing, world: &mut atglyph_bevy::EffectWorld<'_, '_>) {
            if let Some(on) = landing.on {
                world.commands.entity(on).insert(Marked);
            }
        }
    }

    impl atglyph_bevy::FromArgs for Mark {
        const KIND: &'static str = "Mark";
        fn from_args(_: &atglyph_rules::ability::RawValue, _: &Names<'_>) -> Result<Self, String> {
            Ok(Self)
        }
    }

    /// A bag holding a polish whose use asks for kit, three charges of it;
    /// a coat counted as kit and worn; a stone; and boots counted as kit;
    /// with the effects and charges a use resolves through, and the
    /// targeting cursor, which a game's own use key reaches.
    fn chooser() -> (Stage, Entity, Entity, Entity, Entity) {
        let plugins = (atglyph_bevy::EffectsPlugin, crate::TargetViewPlugin, InventoryPanel::new(GridRect::new(0, 0, 52, 14)).title("Bag"));
        let mut stage = Stage::new_with(plugins, |app| {
            app.add_engine_effects();
            atglyph_bevy::EngineApp::add_effect::<Mark>(app);
            let mut registries = app.world_mut().resource_mut::<Registries>();
            registries.tags = Registry::from_defs(vec![atglyph_rules::TagDef::new("kit")]).unwrap();
            registries.slots = Registry::from_defs(vec![SlotDef::new("body")]).unwrap();
        })
        .screen(52, 14);
        let player = stage.player;
        let (kit, body) = {
            let r = stage.app.world().resource::<Registries>();
            (r.tags.expect("kit"), EquipShape::in_slot(r.slots.expect("body")))
        };
        let triggers = {
            let world = stage.app.world();
            let names = world.resource::<Registries>().names();
            let specs: Vec<atglyph_rules::TriggerSpec> =
                names.load_list(r#"[(on: "use", asks: (prompt: "Polish which?", tag: "kit"), effects: [(kind: "Mark")])]"#).expect("the trigger parses");
            atglyph_bevy::Triggers::build(&specs, &[], world.resource::<atglyph_bevy::TriggerKinds>(), world.resource::<EffectKinds>(), &names)
                .expect("it builds")
        };
        let polish = stage.app.world_mut().spawn((Item, Name::new("polish"), triggers, Consumable::new(3, WhenEmpty::Kept))).id();
        let coat = stage.app.world_mut().spawn((Item, Name::new("coat"), Tagged(vec![kit]), Wearable(body.clone()))).id();
        let stone = stage.app.world_mut().spawn((Item, Name::new("stone"))).id();
        let boots = stage.app.world_mut().spawn((Item, Name::new("boots"), Tagged(vec![kit]))).id();
        let mut worn = Equipped(Equipment::with_slot_count(1));
        worn.equip(coat, &body).unwrap();
        stage.app.world_mut().entity_mut(player).insert((Inventory { items: vec![polish, coat, stone, boots] }, worn));
        stage.tick();
        (stage, polish, coat, stone, boots)
    }

    /// A use that asks for a thing opens the bag again choosing, under the
    /// trigger's own prompt and among only what it may be made on; confirm
    /// writes the use on the thing picked out and puts every screen away.
    #[test]
    fn a_use_that_asks_for_a_thing_chooses_under_its_prompt_and_confirm_writes_use_on() {
        let (mut stage, polish, _, _, boots) = chooser();
        stage.press(KeyCode::KeyI);
        stage.press(KeyCode::KeyA);
        stage.press(KeyCode::KeyU);
        assert!(stage.row(0).contains(" Polish which? "), "under the trigger's prompt: {:?}", stage.row(0));
        assert!(
            inside(&stage, 1).starts_with("a) coat") && inside(&stage, 2) == "b) boots",
            "only what it may be made on, lettered in its own order: {:?}",
            stage.rows()
        );
        assert!(stage.row(13).contains("a-b enter choose \u{2022} esc never mind"), "the close key says what it does: {:?}", stage.row(13));
        assert!(stage.app.world().resource::<Modals>().any_open(), "still up, choosing");

        stage.press(KeyCode::ArrowDown);
        stage.press(KeyCode::Enter);
        let uses: Vec<UseOn> = stage.app.world_mut().resource_mut::<Messages<Intent<UseOn>>>().drain().map(|i| i.action).collect();
        assert_eq!(uses, vec![UseOn { item: polish, on: boots }], "the use, on the thing picked out");
        assert!(!stage.app.world().resource::<Modals>().any_open(), "and every screen put away");
    }

    /// While choosing, every plain letter is a choice's: a letter makes its
    /// choice in one press, and the use key, being a letter, confirms
    /// nothing there.
    #[test]
    fn a_letter_while_choosing_makes_its_choice_in_one_press() {
        let (mut stage, polish, _, _, boots) = chooser();
        stage.press(KeyCode::KeyI);
        stage.press(KeyCode::KeyA);
        stage.press(KeyCode::KeyU);
        stage.press(KeyCode::KeyU);
        let uses = |stage: &mut Stage| stage.app.world_mut().resource_mut::<Messages<Intent<UseOn>>>().drain().map(|i| i.action).collect::<Vec<_>>();
        assert!(uses(&mut stage).is_empty(), "u is the twenty-first choice's, and there are two");
        assert!(stage.row(0).contains(" Polish which? "), "still choosing: {:?}", stage.row(0));
        stage.press(KeyCode::KeyB);
        assert_eq!(uses(&mut stage), vec![UseOn { item: polish, on: boots }], "b is the boots, chosen");
        assert!(!stage.app.world().resource::<Modals>().any_open(), "and every screen put away");
    }

    /// The whole of it from the bag: a choice put away spends no charge
    /// and passes no turn, and one made lands on the thing chosen, worn as
    /// it is, for a charge and the turn.
    #[test]
    fn a_choice_put_away_spends_nothing_and_one_made_lands_on_the_thing_chosen() {
        let (mut stage, polish, coat, stone, boots) = chooser();
        let charges = |stage: &Stage| stage.app.world().get::<Consumable>(polish).expect("charges").left;
        let now = |stage: &Stage| stage.app.world().resource::<Turns>().now();
        let marked = |stage: &Stage, thing: Entity| stage.app.world().get::<Marked>(thing).is_some();
        let before = now(&stage);
        stage.press(KeyCode::KeyI);
        stage.press(KeyCode::KeyA);
        stage.press(KeyCode::KeyU);
        stage.press(KeyCode::Escape);
        assert!(!stage.app.world().resource::<Modals>().any_open(), "put away");
        assert_eq!((charges(&stage), now(&stage)), (3, before), "no charge, no time");
        assert!(!marked(&stage, coat) && !marked(&stage, boots), "and nothing done to anything");
        assert!(stage.app.world().get::<MyTurn>(stage.player).is_some(), "and the turn is still the player's");

        stage.press(KeyCode::KeyI);
        assert_eq!(inside(&stage, 1), "a) polish", "the bag opens as it always does, not choosing");
        stage.press(KeyCode::KeyA);
        stage.press(KeyCode::KeyU);
        stage.press(KeyCode::Enter);
        assert!(marked(&stage, coat) && !marked(&stage, boots) && !marked(&stage, stone), "on the coat picked out, worn as it is");
        assert_eq!(charges(&stage), 2, "for a charge");
        assert!(now(&stage) > before, "and the turn");
    }

    /// A game's own use key may write `AimUse` for anything; for a thing
    /// whose use asks for another, the bag opens choosing for it, and no
    /// cursor opens and nothing is used on nothing.
    #[test]
    fn a_games_own_use_key_on_a_thing_that_asks_opens_the_bag_choosing() {
        let (mut stage, polish, _, _, _) = chooser();
        let user = stage.player;
        stage.app.world_mut().write_message(AimUse { user, item: polish });
        stage.tick();
        let modals = stage.app.world().resource::<Modals>();
        assert!(modals.is_top(inventory_modal(modals)), "the bag is up");
        assert!(stage.row(0).contains(" Polish which? "), "choosing: {:?}", stage.row(0));
        assert!(!stage.app.world().resource::<crate::TargetView>().aiming(), "no cursor");
        assert!(stage.app.world_mut().resource_mut::<Messages<Intent<UseItem>>>().drain().next().is_none(), "and nothing used on nothing");
    }

    /// Everything the game heard taken up, and every interaction written,
    /// heard or not.
    #[derive(Resource, Default)]
    struct Taken {
        heard: Vec<Interacted>,
        asked: usize,
    }

    /// Mirela beside the player offering one thing, `give`, for two turns,
    /// that chooses first among what the player carries but a keepsake,
    /// under "Hand over which?"; the player carrying a stone, a keepsake
    /// and a polish, in that order. Returns the stage, Mirela, the stone
    /// and the polish.
    fn giving() -> (Stage, Entity, Entity, Entity) {
        let plugins = (
            atglyph_bevy::PropsPlugin,
            crate::OffersPanel::new(GridRect::new(0, 0, 52, 14)),
            InventoryPanel::new(GridRect::new(0, 0, 52, 14)).never_mind("keep it"),
        );
        let mut stage = Stage::new_with(plugins, |app| {
            app.insert_resource(atglyph_bevy::Seed(atglyph_bevy::testing::TEST_SEED));
            app.add_verb("give");
            app.init_resource::<Taken>()
                .add_systems(atglyph_bevy::Turn, |mut taken: ResMut<Taken>, mut done: MessageReader<Interacted>| taken.heard.extend(done.read().copied()))
                .add_systems(Last, |mut taken: ResMut<Taken>, mut asked: MessageReader<Intent<Interact>>| taken.asked += asked.read().count());
        })
        .screen(52, 14);
        let among = stage.app.register_system(|In(chooser): In<OfferChooser>, bags: Query<&Inventory>, names: Query<&Name>| -> Vec<Entity> {
            let bag = bags.get(chooser.taker).map(|b| b.items.clone()).unwrap_or_default();
            bag.into_iter().filter(|t| names.get(*t).is_ok_and(|n| n.as_str() != "keepsake")).collect()
        });
        let give = stage.app.world().resource::<Verbs>().get("give").expect("declared");
        let [stone, keepsake, polish] = ["stone", "keepsake", "polish"].map(|name| stage.app.world_mut().spawn((Item, Name::new(name))).id());
        let player = stage.player;
        stage.app.world_mut().entity_mut(player).insert(Inventory { items: vec![stone, keepsake, polish] });
        let offers = Offers(vec![Offering::new(give, 200).choosing("Hand over which?", among)]);
        let beside = stage.at.offset(1, 0);
        let mirela = stage.app.world_mut().spawn((Actor, Blocks, Position(beside), OnMap(MapId::ZERO), Name::new("Mirela"), offers)).id();
        stage.tick();
        stage.tick();
        (stage, mirela, stone, polish)
    }

    /// What the stage heard taken up, how many interactions were written,
    /// and the turn clock.
    fn taken(stage: &Stage) -> (Vec<Interacted>, usize, u32) {
        let taken = stage.app.world().resource::<Taken>();
        (taken.heard.clone(), taken.asked, stage.app.world().resource::<Turns>().now())
    }

    /// Walking into Mirela opens the offers screen and takes nothing up;
    /// taking her row up there, or pressing the interact key beside her,
    /// opens the bag under the offer's prompt among only what the game
    /// names, before anything is written or any time passes.
    #[test]
    fn an_offer_that_chooses_first_opens_the_bag_among_what_the_game_names_before_any_time_passes() {
        let (mut stage, _, _, _) = giving();
        let before = taken(&stage);
        let player = stage.player;
        stage.app.world_mut().write_message(Intent::new(player, Bump(Direction::East)));
        stage.tick();
        let modals = stage.app.world().resource::<Modals>();
        assert!(modals.is_open(crate::offers_modal(modals)), "a bump opens the offers screen");
        assert_eq!(taken(&stage), before, "and takes nothing up");
        stage.press(KeyCode::Enter);
        let bag = |stage: &Stage| {
            let modals = stage.app.world().resource::<Modals>();
            (modals.is_top(inventory_modal(modals)), modals.is_open(crate::offers_modal(modals)))
        };
        assert_eq!(bag(&stage), (true, false), "taken up there, the bag in its place");
        assert!(stage.row(0).contains(" Hand over which? "), "under the offer's prompt: {:?}", stage.row(0));
        assert!(stage.row(13).contains("enter choose \u{2022} esc keep it"), "the close key in the game's words: {:?}", stage.row(13));
        assert_eq!((inside(&stage, 1), inside(&stage, 2)), ("a) stone".to_string(), "b) polish".to_string()), "among what the game names: {:#?}", stage.rows());
        assert_eq!(taken(&stage), before, "nothing written, no time passed");

        stage.press(KeyCode::Escape);
        stage.press(KeyCode::Enter);
        assert_eq!(bag(&stage), (true, false), "the interact key opens the bag at once");
        assert_eq!(taken(&stage), before, "and nothing is written");
    }

    /// A pick takes the offer up with the thing picked out, charged the
    /// offer's own time, and puts every screen away.
    #[test]
    fn a_pick_takes_the_offer_up_with_the_thing_picked_out_for_its_own_time() {
        let (mut stage, mirela, _, polish) = giving();
        let (_, _, clock) = taken(&stage);
        stage.press(KeyCode::Enter);
        stage.press(KeyCode::ArrowDown);
        stage.press(KeyCode::Enter);
        let (heard, _, now) = taken(&stage);
        let give = stage.app.world().resource::<Verbs>().get("give").expect("declared");
        assert_eq!(heard, vec![Interacted { actor: stage.player, target: mirela, verb: give, thing: Some(polish) }], "taken up with the polish");
        assert_eq!(now - clock, 200, "for its own time");
        assert!(!stage.app.world().resource::<Modals>().any_open(), "and every screen put away");
    }

    /// Putting the bag away takes nothing up, writes nothing and costs
    /// nothing, and leaves the turn the player's.
    #[test]
    fn backing_out_of_an_offers_choice_writes_nothing_and_costs_nothing() {
        let (mut stage, _, _, _) = giving();
        let before = taken(&stage);
        stage.press(KeyCode::Enter);
        stage.press(KeyCode::Escape);
        assert!(!stage.app.world().resource::<Modals>().any_open(), "put away");
        assert_eq!(taken(&stage), before, "nothing written and no time");
        assert!(stage.app.world().get::<MyTurn>(stage.player).is_some(), "and the turn still the player's");
    }

    /// A pick of a thing that left the pack in the very frame it was
    /// picked, on a bag still listing it, is refused by the engine; and so
    /// is a pick for Mirela once she has walked out of reach. Neither
    /// takes anything up or spends any time.
    #[test]
    fn a_pick_no_longer_carried_or_for_an_offerer_out_of_reach_is_refused_for_nothing() {
        let (mut stage, mirela, stone, polish) = giving();
        let (_, _, clock) = taken(&stage);
        let player = stage.player;
        stage.press(KeyCode::Enter);
        stage.press(KeyCode::ArrowDown);
        stage.app.world_mut().entity_mut(player).insert(Inventory { items: vec![stone] });
        stage.app.world_mut().resource_mut::<atglyph_bevy::testing::KeyScript>().press(KeyCode::Enter);
        stage.tick();
        stage.tick();
        let (heard, asked, now) = taken(&stage);
        assert_eq!((heard.len(), now), (0, clock), "the polish, gone, is refused for nothing");
        assert_eq!(asked, 1, "though it was asked for");

        stage.app.world_mut().entity_mut(player).insert(Inventory { items: vec![stone, polish] });
        stage.tick();
        stage.press(KeyCode::Enter);
        stage.app.world_mut().entity_mut(mirela).insert(Position(stage.at.offset(6, 0)));
        stage.tick();
        stage.tick();
        stage.press(KeyCode::Enter);
        let (heard, _, now) = taken(&stage);
        assert_eq!((heard.len(), now), (0, clock), "out of reach, refused for nothing");
    }

    /// A worn thing that will not come off is offered no key that would
    /// take it off or drop it, says so, and those keys do nothing to it.
    #[test]
    fn a_stuck_thing_worn_offers_no_key_that_would_take_it_off_and_says_it_will_not_come_off() {
        let (mut stage, blade, _, _) = staged();
        stage.app.world_mut().entity_mut(blade).insert(Stuck);
        stage.tick();
        stage.press(KeyCode::KeyI);
        stage.press(KeyCode::KeyA);
        let footer = stage.row(13);
        assert!(!footer.contains("take off") && !footer.contains("drop") && footer.contains("esc back"), "{footer:?}");
        assert!(detail(&stage).iter().any(|l| l == "will not come off"), "{:?}", detail(&stage));
        stage.press(KeyCode::KeyE);
        stage.press(KeyCode::KeyD);
        let w = stage.app.world_mut();
        assert!(w.resource_mut::<Messages<Intent<Unequip>>>().drain().next().is_none(), "nothing taken off");
        assert!(w.resource_mut::<Messages<Intent<DropItem>>>().drain().next().is_none(), "nothing dropped");
        assert!(stage.app.world().resource::<Modals>().any_open(), "and the bag stays up");
    }

    /// A thing not yet known reads as unknown and where it goes and nothing
    /// else, not what its use does nor what it holds, and its use key still
    /// uses it.
    #[test]
    fn an_unknown_thing_reads_as_unknown_and_where_it_goes_and_nothing_else() {
        let (mut stage, triggers) = with_triggers(r#"[(on: "use", effects: [(kind: "Mend", args: (roll: "5"))])]"#);
        stage.app.world_mut().resource_mut::<Registries>().slots = Registry::from_defs(vec![SlotDef::new("finger")]).unwrap();
        let player = stage.player;
        let finger = stage.app.world().resource::<Registries>().slots.expect("finger");
        let held = Consumable { left: 3, ..Consumable::new(5, WhenEmpty::Kept) };
        let draught = stage.app.world_mut().spawn((Item, Name::new("murky draught"), triggers, held, Unknown)).id();
        let ring = stage.app.world_mut().spawn((Item, Name::new("jade ring"), Wearable(EquipShape::in_slot(finger)), Armor(1), Unknown)).id();
        stage.app.world_mut().entity_mut(player).insert(Inventory { items: vec![draught, ring] });
        stage.tick();
        let said = |stage: &Stage| detail(stage)[3..].iter().filter(|l| !l.is_empty()).cloned().collect::<Vec<_>>();

        stage.press(KeyCode::KeyI);
        assert_eq!(said(&stage), ["unknown"], "not what it does, nor what it holds");
        stage.press(KeyCode::ArrowDown);
        assert_eq!(said(&stage), ["unknown", "goes on the finger"], "not its armor, and where it goes");
        stage.press(KeyCode::ArrowUp);
        stage.press(KeyCode::Enter);
        stage.press(KeyCode::KeyU);
        let uses: Vec<Entity> = stage.app.world_mut().resource_mut::<Messages<Intent<UseItem>>>().drain().map(|i| i.action.0).collect();
        assert_eq!(uses, vec![draught], "and it is used as what it is");
    }

    /// A game that knows what a thing not yet known seems says so in place
    /// of the bare `unknown`, line by line in its own tones, and the numbers
    /// it wrote into the row read as a known thing's do; what it seems is
    /// never said of a thing known, and a thing it says nothing of still
    /// reads `unknown`.
    #[test]
    fn what_a_game_says_an_unknown_thing_seems_is_said_in_place_of_unknown() {
        let mut stage = Stage::new_with(InventoryPanel::new(GridRect::new(0, 0, 52, 14)).title("Bag"), |app| {
            app.world_mut().resource_mut::<Registries>().slots = Registry::from_defs(vec![SlotDef::new("body")]).unwrap();
        })
        .screen(52, 14);
        let player = stage.player;
        let body = stage.app.world().resource::<Registries>().slots.expect("body");
        let coat = |name: &str| (Item, Name::new(name.to_string()), Wearable(EquipShape::in_slot(body)), Armor(5));
        let unknown = stage.app.world_mut().spawn((coat("mail"), Unknown)).id();
        let known = stage.app.world_mut().spawn(coat("coat")).id();
        let silent = stage.app.world_mut().spawn((coat("hauberk"), Unknown)).id();
        stage.app.world_mut().entity_mut(player).insert(Inventory { items: vec![unknown, known, silent] });
        stage.app.add_systems(
            Update,
            (move |mut view: ResMut<InventoryView>| {
                for row in view.rows.iter_mut().filter(|r| r.entity != silent) {
                    row.seems.push(crate::Facet::new("seems", "worth not yet known").toned(Tones::MUTED));
                    row.seems.push(crate::Facet::new("seems", "known in 3 turns").toned(Tones::BAD));
                    if row.unknown {
                        row.armor = 3;
                    }
                }
            })
            .in_set(crate::ViewSet::Annotate),
        );
        stage.tick();
        stage.press(KeyCode::KeyI);
        let said = |stage: &Stage| detail(stage)[4..].iter().filter(|l| !l.is_empty()).cloned().collect::<Vec<_>>();
        assert_eq!(said(&stage), ["worth not yet known", "known in 3 turns", "armor +3", "goes on the body"], "in place of the bare word");
        let palette = stage.app.world().resource::<Palette>();
        let tone_at = |y: i32| stage.app.world().resource::<Terminal>().get(2, y).map(|c| c.fg);
        assert_eq!((tone_at(5), tone_at(6)), (Some(palette.get(Tones::MUTED)), Some(palette.get(Tones::BAD))), "each in its own tone");
        stage.press(KeyCode::ArrowDown);
        assert_eq!(said(&stage), ["armor +5", "goes on the body"], "a thing known needs no word on what it seems");
        stage.press(KeyCode::ArrowDown);
        assert_eq!(said(&stage), ["unknown", "goes on the body"], "and one the game said nothing of is unknown");
    }

    /// A thing not yet known that can be thrown says nothing of how far it
    /// flies or what it strikes for, and its key still throws it.
    #[test]
    fn an_unknown_thing_that_can_be_thrown_shows_no_reach_and_is_still_thrown() {
        let (mut stage, _, _, knives) = staged();
        stage.app.world_mut().entity_mut(knives).insert(Unknown);
        stage.tick();
        stage.press(KeyCode::KeyI);
        stage.press(KeyCode::ArrowDown);
        stage.press(KeyCode::ArrowDown);
        assert_eq!(inside(&stage, 3), "c) 3 knives", "by its name and how many");
        let said: Vec<String> = detail(&stage)[4..].iter().filter(|l| !l.is_empty()).cloned().collect();
        assert_eq!(said, ["unknown"], "not how far it flies, nor what it strikes for");
        stage.press(KeyCode::Enter);
        assert!(stage.row(13).contains("t throw"), "{:?}", stage.row(13));
        stage.press(KeyCode::KeyT);
        let thrown: Vec<Entity> = stage.app.world_mut().resource_mut::<Messages<AimThrow>>().drain().map(|a| a.item).collect();
        assert_eq!(thrown, vec![knives], "and it is thrown as what it is");
    }

    /// The last turn of a charge is one turn, not one turns.
    #[test]
    fn a_plate_a_turn_from_ready_says_one_turn() {
        let (mut stage, triggers) = with_triggers(r#"[(on: "use", effects: [(kind: "Mend", args: (roll: "2"))])]"#);
        let player = stage.player;
        let nearly = Consumable { left: 0, recharge: Some(atglyph_bevy::Recharge { every: 4000, progress: 3950 }), ..Consumable::new(1, WhenEmpty::Kept) };
        let wand = stage.app.world_mut().spawn((Item, Name::new("wand"), triggers, nearly)).id();
        stage.app.world_mut().entity_mut(player).insert(Inventory { items: vec![wand] });
        stage.tick();

        stage.press(KeyCode::KeyI);
        assert!(detail(&stage).iter().any(|l| l == "ready in 1 turn"), "{:?}", detail(&stage));
    }
}
