//! The bag: what you carry, what each thing is worth, and what to do with
//! it.
//!
//! A modal the engine runs end to end, the way the ability menu is:
//! [`InventoryKeys::toggle`] opens and closes it, the direction keys walk
//! the rows, and four keys act on the row picked out. Wearing, dropping and
//! using are the engine's own item actions, so the screen writes their
//! intents itself; throwing opens the targeting cursor through
//! [`AimThrow`], the way a game's throw key would. A game binds nothing.
//! Using an item with a `use` trigger uses it where it stands, and using
//! one whose use is aimed opens the targeting cursor through [`AimUse`], as
//! throwing does, and puts it away again for nothing. Using one whose use
//! asks for another of the player's things turns the bag to choosing,
//! titled with the use's own prompt and listing only what it may be made
//! on: confirm writes [`UseOn`] for the thing picked out, and putting the
//! bag away spends nothing; a game's own use key writing [`AimUse`] for
//! such a thing opens the bag choosing the same way. A game's own question
//! no use asks, which thing to hand over, is an [`AskForThing`]: the bag
//! chooses the same way among what it names, and confirm answers with
//! [`ThingChosen`], which the game acts on. Nothing else is used
//! from here; an item a game answers itself from [`ItemEvent::Used`] is
//! used by the game's own key, since the bag cannot know it does anything
//! and a use that did nothing would spend a turn. The footer offers only
//! the keys that do something to the row picked out: wear or take off,
//! drop, use, throw, and close, and for a worn thing that will not come
//! off, none that would take it off.
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
//! Every action closes every screen, since it spends a turn and the turn
//! loop assumes nothing is up while it runs.

use crate::modal::AddModal;
use bevy::prelude::*;
use rl_bevy::prelude::*;
use rl_bevy::{EngineSet, PresentSet};
use rl_core::{Direction, Rect};
use rl_render::{Cell, Terminal};

use crate::controls::{AddControls, Chord, ControlInput, EngineKey, key_name};
use crate::modal::{ModalId, Modals};
use crate::panel::ability::reach;
use crate::panel::sheet::plain_op;
use crate::panel::{clear, clip, frame, wrap};
use crate::tone::{Palette, ToneId, Tones};
use crate::view::ability::turns;
use crate::view::inventory::{InventoryView, InventoryViewPlugin, ItemRow};
use crate::view::sheet::Strike;
use crate::view::target::{AimThrow, AimUse};

/// The name the bag's modal is declared under.
pub const INVENTORY_MODAL: &str = "inventory";

/// The keys the bag answers to. Walking the rows and closing use the
/// direction and cursor keys every other screen uses, and the cursors'
/// confirm key uses the row picked out as well.
#[derive(Resource, Debug, Clone)]
pub struct InventoryKeys {
    /// Opens and closes the bag.
    pub toggle: Chord,
    /// Puts the row picked out on, or takes it off.
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
pub struct InventoryLayout {
    /// The terminal cells it occupies.
    pub rect: Rect,
    /// Drawn in the top border. The engine has no word for a sea chest.
    pub title: String,
    /// What the bag is called where the engine names it, on the controls
    /// screen.
    pub what: String,
    /// Shown in place of rows when there are none.
    pub empty: String,
}

/// Which row the cursor is on, and what the bag is choosing for. The
/// screen's own state, since a view holds no cursor.
#[derive(Resource, Debug, Default)]
pub struct InventoryMenu {
    /// The row picked out, clamped to the rows there are when drawn.
    pub selected: usize,
    /// While the bag is choosing what a use is made on: the thing used, and
    /// which of its choices the cursor is on. Put away whenever the bag is,
    /// so it never opens choosing for a use nobody asked for again.
    pub choosing: Option<ChoosingFor>,
    /// While the bag is choosing for a game's own question: the question,
    /// and which of its choices the cursor is on. Put away whenever the
    /// bag is, as a use's choosing is.
    pub asked: Option<Asked>,
}

impl InventoryMenu {
    /// Moves the cursor `by` rows over `rows` of them, wrapping at either
    /// end.
    pub fn move_by(&mut self, by: i32, rows: usize) {
        self.selected = stepped(self.selected, by, rows);
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

/// A game's question that the bag answers by choosing one of the player's
/// things: which to hand over, which to offer up.
///
/// The sibling of a use that asks for a thing, for a question no use of a
/// thing asks. A game writes it, most often when an offer of its own is
/// taken up, as a trade taken up opens the trade screen: the bag opens
/// choosing among `choices` under `prompt`, confirm writes [`ThingChosen`]
/// for the thing picked out and puts every screen away, and putting the
/// bag away answers nothing. What the choice does is the game's: the
/// engine moves nothing and spends no time, since the offer that asked
/// has spent its own. Opened only for the player the bag shows and only
/// with no screen up, as a game's use key opens nothing over another. A
/// choice is answered only while the asker still offers `verb` to the
/// user, alive and in reach: one gone by then is answered nothing.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct AskForThing {
    /// Who chooses: the player the bag shows.
    pub user: Entity,
    /// Who or what asked, for the game to answer: the one the offer was
    /// taken up on.
    pub asker: Entity,
    /// What for: the verb of the offer taken up, so two questions put by
    /// one asker are told apart.
    pub verb: VerbId,
    /// What the bag is titled while it chooses.
    pub prompt: String,
    /// What it may choose among, in the order listed. A thing the bag does
    /// not hold is left out, and with nothing left the bag says it is
    /// empty and choosing it answers nothing.
    pub choices: Vec<Entity>,
}

/// The thing the player chose for an [`AskForThing`]: the question's own
/// `user`, `asker` and `verb`, and what was picked out.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThingChosen {
    /// Who chose.
    pub user: Entity,
    /// Who or what asked.
    pub asker: Entity,
    /// What for.
    pub verb: VerbId,
    /// The thing chosen.
    pub thing: Entity,
}

/// The bag choosing for a game's [`AskForThing`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked {
    /// The question.
    pub ask: AskForThing,
    /// Which of its choices the cursor is on, clamped to the choices there
    /// are when drawn.
    pub selected: usize,
}

/// What the bag is choosing among, with no screen of its own: a use's
/// choices or a game's question's, each only while the bag holds it.
fn choices_of(menu: &InventoryMenu, view: &InventoryView) -> Option<(String, Vec<Entity>, usize)> {
    let held = |things: &[Entity]| -> Vec<Entity> { things.iter().copied().filter(|t| view.rows.iter().any(|r| r.entity == *t)).collect() };
    if let Some(asked) = &menu.asked {
        return Some((asked.ask.prompt.clone(), held(&asked.ask.choices), asked.selected));
    }
    let choosing = menu.choosing?;
    let asks = view.rows.iter().find(|r| r.entity == choosing.item)?.asks.as_ref()?;
    Some((asks.prompt.clone(), held(&asks.choices), choosing.selected))
}

/// Moves the cursor of whichever choosing the bag holds to `at`.
fn choose_at(menu: &mut InventoryMenu, at: usize) {
    if let Some(asked) = menu.asked.as_mut() {
        asked.selected = at;
    } else if let Some(choosing) = menu.choosing.as_mut() {
        choosing.selected = at;
    }
}

/// Puts away whatever the bag was choosing for.
fn stop_choosing(menu: &mut InventoryMenu) {
    menu.choosing = None;
    menu.asked = None;
}

/// `at` moved `by` rows over `rows` of them, wrapping at either end.
fn stepped(at: usize, by: i32, rows: usize) -> usize {
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
    pub fn new(rect: Rect) -> Self {
        Self(InventoryLayout { rect, title: "Inventory".into(), what: "inventory".into(), empty: "Nothing.".into() })
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
            .add_message::<AskForThing>()
            .add_message::<ThingChosen>();
        app.insert_resource(self.0.clone()).init_resource::<InventoryMenu>().init_resource::<InventoryKeys>();
        // A game's own use key, or its own question, opens the bag choosing
        // before the bag reads its keys, so the press that asked is not
        // read again inside it.
        app.add_systems(Update, (choose_when_asked, choose_for_the_game, inventory_keys).chain().in_set(EngineSet::Input))
            .add_systems(Update, draw_inventory.in_set(PresentSet::Overlay));
    }

    fn finish(&self, app: &mut App) {
        rl_bevy::depends_on::<crate::UiPlugin>(app, "InventoryPanel");
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

/// What the bag can ask for, and what an answer to a game's question is
/// checked against.
#[derive(bevy::ecs::system::SystemParam)]
pub struct BagIntents<'w> {
    equips: MessageWriter<'w, Intent<Equip>>,
    unequips: MessageWriter<'w, Intent<Unequip>>,
    drops: MessageWriter<'w, Intent<DropItem>>,
    uses: MessageWriter<'w, Intent<UseItem>>,
    uses_on: MessageWriter<'w, Intent<UseOn>>,
    aimed_uses: MessageWriter<'w, AimUse>,
    throws: MessageWriter<'w, AimThrow>,
    chosen: MessageWriter<'w, ThingChosen>,
    offered: Option<Res<'w, OfferedHere>>,
}

impl BagIntents<'_> {
    /// Whether `ask`'s asker still offers its verb to `user`, alive and in
    /// reach. A game with no offers at all asked from somewhere else, and
    /// is answered: there is nothing here to check it against.
    fn still_asked(&self, user: Entity, ask: &AskForThing) -> bool {
        self.offered.as_deref().is_none_or(|offered| offered.find(user, ask.asker, ask.verb).is_some())
    }
}

/// Opens the bag choosing for a thing a game's own use key asked to use,
/// written as [`AimUse`] for anything, when its use asks for another of the
/// player's things: what `AimUse` means for such a thing, as the targeting
/// cursor answers it for one whose use is aimed.
///
/// Its own system because the bag's keys write `AimUse` for an aimed
/// thing, and one system may not read the messages it writes. Only with no
/// screen up, as a game's key opens nothing over another.
pub fn choose_when_asked(mut asked: MessageReader<AimUse>, view: Res<InventoryView>, mut menu: ResMut<InventoryMenu>, mut modals: ResMut<Modals>) {
    let modal = inventory_modal(&modals);
    for asked in asked.read() {
        let wants_a_thing = view.rows.iter().any(|r| r.entity == asked.item && r.asks.is_some() && r.usable());
        if view.entity == Some(asked.user) && wants_a_thing && !modals.any_open() {
            menu.asked = None;
            menu.choosing = Some(ChoosingFor { item: asked.item, selected: 0 });
            modals.open(modal);
        }
    }
}

/// Opens the bag choosing for a game's [`AskForThing`], for the player it
/// shows and only with no screen up.
pub fn choose_for_the_game(mut asked: MessageReader<AskForThing>, view: Res<InventoryView>, mut menu: ResMut<InventoryMenu>, mut modals: ResMut<Modals>) {
    let modal = inventory_modal(&modals);
    for ask in asked.read() {
        if view.entity == Some(ask.user) && !modals.any_open() {
            menu.choosing = None;
            menu.asked = Some(Asked { ask: ask.clone(), selected: 0 });
            modals.open(modal);
        }
    }
}

/// Opens, closes and walks the bag, and acts on the row picked out.
///
/// An action is taken only while the player holds the turn, since an intent
/// written for anyone else is dropped unread; the screen stays up until it
/// does. A use that asks for another of the player's things turns the bag
/// to choosing among what it may be made on, under its prompt: confirm uses
/// it on the thing picked out, and putting the bag away spends nothing. A
/// game's question chooses the same way, and confirm answers it with
/// [`ThingChosen`].
pub fn inventory_keys(
    keys: ControlInput,
    binds: Res<InventoryKeys>,
    view: Res<InventoryView>,
    mut menu: ResMut<InventoryMenu>,
    mut modals: ResMut<Modals>,
    holding: Query<(), (With<Player>, With<MyTurn>)>,
    mut intents: BagIntents,
) {
    let modal = inventory_modal(&modals);
    // A choice lasts only while the bag is up, however it was put away.
    if !modals.is_open(modal) {
        stop_choosing(&mut menu);
    }
    if binds.toggle.just_pressed(keys.input()) && (modals.is_top(modal) || !modals.any_open()) {
        modals.toggle(modal);
        stop_choosing(&mut menu);
        return;
    }
    // Not in the frame it went up: a game may open the bag on a key the
    // bag reads too, and that press is the game's.
    if !modals.is_top(modal) || modals.just_opened() {
        return;
    }
    let bindings = keys.bindings();
    let input = keys.input();
    if input.just_pressed(bindings.cursor.close) {
        modals.close_one(modal);
        stop_choosing(&mut menu);
        return;
    }
    let confirmed = binds.use_it.just_pressed(input) || input.just_pressed(bindings.cursor.confirm) || input.just_pressed(bindings.cursor.also_confirm);
    if let Some((_, choices, selected)) = choices_of(&menu, &view) {
        let by = match bindings.directions.just_pressed(input) {
            Some(Direction::North) => -1,
            Some(Direction::South) => 1,
            _ => 0,
        };
        let selected = stepped(selected.min(choices.len().saturating_sub(1)), by, choices.len());
        choose_at(&mut menu, selected);
        let (Some(user), Some(&on)) = (view.entity, choices.get(selected)) else { return };
        if confirmed && holding.contains(user) {
            match (&menu.asked, menu.choosing) {
                // Only while whoever asked still offers what asked to the
                // user: one that died, or went out of reach while the bag
                // was up, is answered nothing, as the offers screen's
                // rows go with what offered them.
                (Some(asked), _) if intents.still_asked(user, &asked.ask) => {
                    intents.chosen.write(ThingChosen { user, asker: asked.ask.asker, verb: asked.ask.verb, thing: on });
                }
                (Some(_), _) => {}
                (None, Some(choosing)) => {
                    intents.uses_on.write(Intent::new(user, UseOn { item: choosing.item, on }));
                }
                (None, None) => {}
            }
            stop_choosing(&mut menu);
            modals.close_all();
        }
        return;
    }
    match bindings.directions.just_pressed(input) {
        Some(Direction::North) => menu.move_by(-1, view.rows.len()),
        Some(Direction::South) => menu.move_by(1, view.rows.len()),
        _ => {}
    }
    let (Some(user), Some(row)) = (view.entity, view.row(menu.selected)) else { return };
    if !holding.contains(user) {
        return;
    }
    let item = row.entity;
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
    } else if binds.drop.just_pressed(input) && !row.stuck {
        intents.drops.write(Intent::new(user, DropItem(item)));
        true
    } else if confirmed {
        // What does something when used, with a charge to spend. Anything
        // else is not used from here: it would spend a turn on nothing, and
        // the footer never offers it. A use pointed somewhere closes the
        // bag and opens the targeting cursor in its place, as a throw does,
        // and one made on a thing keeps the bag up, choosing for it.
        match (row.usable(), row.targeting, row.asks.is_some()) {
            (true, _, true) => {
                menu.choosing = Some(ChoosingFor { item, selected: 0 });
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
    }
}

/// The keys that do something to `row`, and the one that closes the bag.
///
/// Only what applies, so the footer never offers to wear a pebble, or to
/// take off, drop or throw a worn thing that will not come off. Short
/// enough for a narrow frame: the direction keys walk the rows on every
/// screen and need no saying here.
fn hints(row: Option<&ItemRow>, binds: &InventoryKeys, confirm: String, close: String) -> String {
    let mut keys = Vec::new();
    if let Some(row) = row {
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
    }
    keys.push(close);
    keys.join(" \u{2022} ")
}

/// A strike in words: `1d6 cutlass`, or `1d8 pistol to 6` with a reach.
fn strike(s: &Strike) -> String {
    match s.range {
        Some(range) => format!("{} {} to {range}", s.dice, s.kind),
        None => format!("{} {}", s.dice, s.kind),
    }
}

/// The lines describing `row`: its numbers and what the game added, then
/// where it goes and whether it will come off, then what it does and has
/// left. A thing not yet known says so in place of its numbers and of what
/// it does and has left, and is described by the rest.
fn describe(row: &ItemRow, width: usize) -> Vec<(String, ToneId)> {
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
    if row.armor != 0 {
        say(format!("armor {:+}", row.armor), Tones::TEXT);
    }
    for r in &row.resists {
        match r.pct {
            pct if pct < 0 => say(format!("weak to {} {}%", r.name, -pct), Tones::TEXT),
            pct => say(format!("resists {} {pct}%", r.name), Tones::TEXT),
        }
    }
    for (stat, op) in &row.bestows {
        say(format!("{stat} {}", plain_op(*op)), Tones::TEXT);
    }
    match (&row.thrown, row.throw_range) {
        (Some(thrown), _) => say(format!("thrown {}", strike(thrown)), Tones::TEXT),
        (None, Some(range)) if !row.unknown => say(format!("thrown to {range}"), Tones::TEXT),
        (None, _) => {}
    }
    // What the game added is a fact about the thing the engine could not
    // know, a sight it lends or why a gun cannot fire, so it reads with the
    // numbers rather than under what the thing does and has left.
    for facet in &row.facets {
        say(facet.text.clone(), facet.tone);
    }
    // A two-hander names the hand it empties as well, since putting it on
    // takes whatever is there off.
    let also = row.also_takes.iter().map(|slot| format!("the {slot}")).collect::<Vec<_>>().join(" and ");
    if row.worn() {
        let also = if also.is_empty() { also } else { format!(" and {also}") };
        say(format!("worn on the {}{also}", row.slot_name), Tones::MUTED);
    } else if row.wearable() {
        let also = if also.is_empty() { also } else { format!(", and takes {also}") };
        say(format!("goes on the {}{also}", row.goes_on.join(" or the ")), Tones::MUTED);
    }
    // Why the footer offers nothing that would take it off.
    if row.stuck {
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
        (false, true, Some(whole)) => say(format!("ready in {}", turns(whole * rl_core::turn::BASE_ACTION_COST)), Tones::MUTED),
        (false, true, None) => say("empty".to_string(), Tones::MUTED),
        (false, false, _) => {}
    }
    lines
}

/// What the bag is drawn from.
#[derive(bevy::ecs::system::SystemParam)]
pub struct BagScreen<'w> {
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
pub fn draw_inventory(mut terminal: ResMut<Terminal>, mut menu: ResMut<InventoryMenu>, screen: BagScreen, mut top: Local<usize>) {
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
            (prompt, rows, at, format!("{confirm} choose \u{2022} {close}"))
        }
        None => {
            menu.selected = menu.selected.min(view.rows.len().saturating_sub(1));
            (layout.title.clone(), view.rows.iter().collect(), menu.selected, hints(view.row(menu.selected), binds, confirm, close))
        }
    };
    clear(&mut terminal, rect, palette);
    frame(&mut terminal, rect, &title, &hints, palette);
    let inner = Rect::new(rect.x + 2, rect.y + 1, rect.width - 4, rect.height - 2);
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
        let fg = palette.get(if row.worn() { Tones::NOTICE } else { Tones::TEXT });
        terminal.fill(Rect::new(inner.x, y, inner.width, 1), Cell::new(' ', fg).on(bg));
        // The item's own glyph first, in its own colour, then its name; a
        // stack says how many.
        let mut x = inner.x;
        if let Some(glyph) = row.glyph {
            terminal.set(x, y, Cell::new(glyph.ch, glyph.fg).on(bg));
            x += 2;
        }
        let label = rl_core::noun::listed(&row.label, row.count);
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
    for (y, (line, tone)) in (rule_y + 1..inner.bottom()).zip(describe(row, width)) {
        terminal.print_on(inner.x, y, &clip(&line, width), palette.get(tone), surface);
    }
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
    use rl_bevy::{AddEngineEffects, Intent, ThrowingPlugin};
    use rl_core::DiceRoll;
    use rl_rules::Names;
    use rl_rules::content::Registry;
    use rl_rules::{EquipShape, Equipment, SlotDef};

    /// A player with a worn blade, a hat in the bag and a stack of knives,
    /// and the bag drawn in a small frame.
    fn staged() -> (Stage, Entity, Entity, Entity) {
        let mut stage =
            Stage::new_with((ThrowingPlugin, crate::TargetViewPlugin, InventoryPanel::new(Rect::new(0, 0, 52, 14)).title("Bag").empty("Lint.")), |app| {
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
                rl_render::Glyph::new(')', Color::WHITE),
                Wearable(EquipShape::in_slot(hand)),
                MeleeAttack::new(kind, DiceRoll::new(1, 6)),
            ))
            .id();
        let mut resists = rl_rules::Resistances::new();
        resists.set(kind, 10);
        let hat = stage.app.world_mut().spawn((Item, Name::new("a hat"), Wearable(EquipShape::in_slot(head)), Armor(1), Resists(resists))).id();
        let knives =
            stage.app.world_mut().spawn((Item, Name::new("knife"), Stack { key: 1, count: 3 }, Throwable::new(5, Some((kind, DiceRoll::new(1, 4)))))).id();
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
        let mut stage = Stage::new_with(InventoryPanel::new(Rect::new(0, 0, 52, 14)).title("Bag"), |app| {
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
        let mut stage = Stage::new(InventoryPanel::new(Rect::new(0, 0, 40, 10)).title("Bag")).screen(40, 10);
        let player = stage.player;
        let pebbles: Vec<Entity> = (0..10).map(|i| stage.app.world_mut().spawn((Item, Name::new(format!("pebble {i}")))).id()).collect();
        stage.app.world_mut().get_mut::<Inventory>(player).unwrap().items = pebbles;
        stage.tick();
        stage.press(KeyCode::KeyI);
        let listed = |stage: &Stage| (1..5).map(|y| inside(stage, y)).collect::<Vec<_>>();
        assert_eq!(listed(&stage), ["pebble 0", "pebble 1", "pebble 2", "pebble 3"]);
        assert!(!stage.row(0).contains('\u{25b2}'), "nothing above the first row: {:?}", stage.row(0));
        assert!(stage.row(5).contains(" \u{25bc} 6 more "), "six below, on the rule: {:?}", stage.row(5));

        for _ in 0..4 {
            stage.press(KeyCode::ArrowDown);
        }
        assert_eq!(listed(&stage), ["pebble 1", "pebble 2", "pebble 3", "pebble 4"], "down past the foot moves the list by one");
        assert!(stage.row(0).contains(" \u{25b2} 1 more ") && stage.row(0).contains(" Bag "), "one above, beside the title: {:?}", stage.row(0));
        assert!(stage.row(5).contains(" \u{25bc} 5 more "), "{:?}", stage.row(5));
        stage.press(KeyCode::ArrowUp);
        assert_eq!(listed(&stage), ["pebble 1", "pebble 2", "pebble 3", "pebble 4"], "back up inside the list, it holds still");

        // A game that picks a row itself has it brought into view.
        stage.app.world_mut().resource_mut::<InventoryMenu>().selected = 9;
        stage.tick();
        assert_eq!(listed(&stage), ["pebble 6", "pebble 7", "pebble 8", "pebble 9"]);
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
        assert!(first.starts_with(") a blade") && first.ends_with("hand"), "glyph, name, and the slot it is worn in at the right: {first:?}");
        assert_eq!(inside(&stage, 2), "a hat");
        assert_eq!(inside(&stage, 3), "3 knives", "a stack says how many");
        assert!(stage.row(13).contains("e take off \u{2022} d drop \u{2022} esc"), "the worn blade can come off or be dropped: {:?}", stage.row(13));
        assert_eq!(inside(&stage, 5), "1d6 kinetic", "the blade's own blow");
        assert_eq!(inside(&stage, 6), "worn on the hand");

        stage.press(KeyCode::ArrowDown);
        assert_eq!(inside(&stage, 5), "armor +1");
        assert_eq!(inside(&stage, 6), "resists kinetic 10%", "what it resists, under its armor");
        assert_eq!(inside(&stage, 7), "goes on the head");
        assert!(stage.row(13).contains("e wear \u{2022} d drop \u{2022} esc"), "the hat goes on: {:?}", stage.row(13));
        stage.press(KeyCode::ArrowDown);
        assert_eq!(inside(&stage, 5), "thrown 1d4 kinetic to 5");
        assert!(
            stage.row(13).contains("d drop \u{2022} t throw \u{2022} esc") && !stage.row(13).contains("wear"),
            "knives are thrown, never worn: {:?}",
            stage.row(13)
        );

        stage.press(KeyCode::Escape);
        assert!(!stage.app.world().resource::<Modals>().any_open());
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
            (move |mut view: ResMut<InventoryView>, mut facets: ResMut<crate::Facets>| {
                for row in view.rows.iter_mut().filter(|r| r.entity == hat) {
                    row.facets.push(facets.facet("dark sight", "sees 8 in the dark"));
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
    fn the_keys_wear_drop_use_and_throw_the_row_picked_out() {
        let (mut stage, blade, hat, knives) = staged();
        let player = stage.player;
        let drain = |stage: &mut Stage| {
            let w = stage.app.world_mut();
            let unequips: Vec<Entity> = w.resource_mut::<Messages<Intent<Unequip>>>().drain().map(|i| i.action.0).collect();
            let equips: Vec<Entity> = w.resource_mut::<Messages<Intent<Equip>>>().drain().map(|i| i.action.0).collect();
            let drops: Vec<Entity> = w.resource_mut::<Messages<Intent<DropItem>>>().drain().map(|i| i.action.0).collect();
            let uses: Vec<Entity> = w.resource_mut::<Messages<Intent<UseItem>>>().drain().map(|i| i.action.0).collect();
            let throws: Vec<Entity> = w.resource_mut::<Messages<AimThrow>>().drain().map(|a| a.item).collect();
            (unequips, equips, drops, uses, throws)
        };
        let closed = |stage: &Stage| !stage.app.world().resource::<Modals>().is_open(inventory_modal(stage.app.world().resource::<Modals>()));

        stage.press(KeyCode::KeyI);
        stage.press(KeyCode::KeyE);
        assert_eq!(drain(&mut stage).0, vec![blade], "the worn blade is taken off");
        assert!(closed(&stage));
        stage.tick();
        assert!(stage.app.world().get::<Equipped>(player).unwrap().slot_of(blade).is_none(), "and the engine resolved it");

        stage.press(KeyCode::KeyI);
        stage.press(KeyCode::ArrowDown);
        stage.press(KeyCode::KeyE);
        assert_eq!(drain(&mut stage).1, vec![hat], "the hat goes on");
        assert!(closed(&stage));

        stage.press(KeyCode::KeyI);
        stage.press(KeyCode::KeyD);
        assert_eq!(drain(&mut stage).2, vec![hat], "the cursor stayed on the hat, which is dropped");
        stage.tick();
        stage.press(KeyCode::KeyI);
        assert_eq!(inside(&stage, 2), "3 knives", "and is out of the bag");
        stage.press(KeyCode::Enter);
        assert!(drain(&mut stage).3.is_empty(), "knives lend nothing to use, so confirm does nothing and spends no turn");
        assert!(!closed(&stage), "and the bag stays up");

        stage.press(KeyCode::ArrowUp);
        stage.press(KeyCode::KeyT);
        assert!(drain(&mut stage).4.is_empty(), "a blade is not thrown");
        assert!(!closed(&stage), "so the bag stays up");
        stage.press(KeyCode::ArrowDown);
        stage.press(KeyCode::KeyT);
        assert_eq!(drain(&mut stage).4, vec![knives], "the knives are");
        assert!(closed(&stage), "and the bag closed for the cursor");
    }

    /// A game that opens the bag on its own key, one the bag also reads,
    /// opens it and no more: the press that put the screen up is not read
    /// again inside it, whichever of the two systems runs first.
    #[test]
    fn a_game_key_that_opens_the_bag_is_not_also_read_by_it() {
        let (mut stage, blade, hat, knives) = staged();
        let player = stage.player;
        stage.app.world_mut().entity_mut(player).insert(Inventory { items: vec![knives, blade, hat] });
        stage.app.add_systems(
            Update,
            (|keys: Res<ButtonInput<KeyCode>>, mut modals: ResMut<Modals>| {
                if keys.just_pressed(KeyCode::KeyT) && !modals.any_open() {
                    let bag = inventory_modal(&modals);
                    modals.open(bag);
                }
            })
            .in_set(rl_bevy::EngineSet::Input)
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
    fn with_triggers(triggers: &str) -> (Stage, rl_bevy::Triggers) {
        let stage = Stage::new_with((rl_bevy::EffectsPlugin, InventoryPanel::new(Rect::new(0, 0, 52, 14)).title("Bag")), |app| {
            app.add_engine_effects();
        })
        .screen(52, 14);
        let specs: Vec<rl_rules::TriggerSpec> = Names::new().load_list(triggers).expect("the triggers parse");
        let built = {
            let world = stage.app.world();
            let registries = world.resource::<Registries>().clone();
            rl_bevy::Triggers::build(&specs, &[], world.resource::<rl_bevy::Moments>(), world.resource::<EffectKinds>(), &registries.names())
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
        assert_eq!(inside(&stage, 1), "2 poultices");
        assert!(detail(&stage).iter().any(|l| l == "use: mends 5"), "described by what it does: {:?}", detail(&stage));
        assert!(stage.app.world().resource::<InventoryView>().rows[0].usable(), "and the row knows it is used");

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
            stage.app.world_mut().spawn((Item, Name::new("grenade"), triggers, Consumable::new(1, WhenEmpty::Destroyed), Throwable::new(6, None))).id();
        stage.app.world_mut().entity_mut(player).insert(Inventory { items: vec![grenade] });
        stage.tick();

        stage.press(KeyCode::KeyI);
        assert!(detail(&stage).iter().any(|l| l == "on landing: 3 kinetic in a burst of 1"), "{:?}", detail(&stage));
        assert!(!stage.app.world().resource::<InventoryView>().rows[0].usable(), "nothing to use");
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
        let charging = Consumable { left: 0, recharge: Some(rl_bevy::Recharge { every: 4000, progress: 1000 }), ..Consumable::new(1, WhenEmpty::Kept) };
        let plate = |name: &str| (Item, Name::new(name.to_string()), triggers.clone(), charging, rl_bevy::Attuned, Wearable(EquipShape::in_slot(torso)));
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
        let mut stage =
            Stage::new_with((rl_bevy::EffectsPlugin, ConsumablesPlugin, crate::TargetViewPlugin, InventoryPanel::new(Rect::new(0, 0, 52, 14))), |app| {
                app.add_engine_effects();
            })
            .screen(52, 14);
        let specs: Vec<rl_rules::TriggerSpec> = Names::new()
            .load_list(r#"[(on: "use", targeting: (mode: Bolt(range: 6)), effects: [(kind: "Harm", args: (kind: "kinetic", roll: "4"))])]"#)
            .expect("the trigger parses");
        let triggers = {
            let world = stage.app.world();
            let names = world.resource::<Registries>().names();
            rl_bevy::Triggers::build(&specs, &[], world.resource::<rl_bevy::Moments>(), world.resource::<EffectKinds>(), &names).expect("it builds")
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
        stage.press(KeyCode::KeyU);
        let view = stage.app.world().resource::<crate::TargetView>();
        assert_eq!((view.using, view.cursor), (Some(rod), stage.at.offset(3, 0)), "the cursor opened on the rat");
        stage.press(KeyCode::Escape);
        assert!(!stage.app.world().resource::<crate::TargetView>().aiming() && !stage.app.world().resource::<Modals>().any_open(), "put away");
        assert_eq!((charges(&stage), now(&stage), hp(&stage, foe)), (3, before, 10), "no charge, no time, no harm");
        assert!(stage.app.world().get::<MyTurn>(player).is_some(), "and the turn is still the player's");

        stage.press(KeyCode::KeyI);
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

    impl rl_bevy::Effect for Mark {
        fn apply(&self, landing: &rl_bevy::Landing, world: &mut rl_bevy::EffectWorld<'_, '_>) {
            if let Some(on) = landing.on {
                world.commands.entity(on).insert(Marked);
            }
        }
    }

    impl rl_bevy::FromArgs for Mark {
        const KIND: &'static str = "Mark";
        fn from_args(_: &rl_rules::ability::RawValue, _: &Names<'_>) -> Result<Self, String> {
            Ok(Self)
        }
    }

    /// A bag holding a polish whose use asks for kit, three charges of it;
    /// a coat counted as kit and worn; a stone; and boots counted as kit;
    /// with the effects and charges a use resolves through, and the
    /// targeting cursor, which a game's own use key reaches.
    fn chooser() -> (Stage, Entity, Entity, Entity, Entity) {
        let plugins = (rl_bevy::EffectsPlugin, ConsumablesPlugin, crate::TargetViewPlugin, InventoryPanel::new(Rect::new(0, 0, 52, 14)).title("Bag"));
        let mut stage = Stage::new_with(plugins, |app| {
            app.add_engine_effects();
            rl_bevy::AddEffect::add_effect::<Mark>(app);
            let mut registries = app.world_mut().resource_mut::<Registries>();
            registries.tags = Registry::from_defs(vec![rl_rules::TagDef::new("kit")]).unwrap();
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
            let specs: Vec<rl_rules::TriggerSpec> =
                names.load_list(r#"[(on: "use", asks: (prompt: "Polish which?", tag: "kit"), effects: [(kind: "Mark")])]"#).expect("the trigger parses");
            rl_bevy::Triggers::build(&specs, &[], world.resource::<rl_bevy::Moments>(), world.resource::<EffectKinds>(), &names).expect("it builds")
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
        stage.press(KeyCode::KeyU);
        assert!(stage.row(0).contains(" Polish which? "), "under the trigger's prompt: {:?}", stage.row(0));
        assert!(inside(&stage, 1).starts_with("coat") && inside(&stage, 2) == "boots", "only what it may be made on: {:?}", stage.rows());
        assert!(stage.row(13).contains("enter choose \u{2022} esc"), "{:?}", stage.row(13));
        assert!(stage.app.world().resource::<Modals>().any_open(), "still up, choosing");

        stage.press(KeyCode::ArrowDown);
        stage.press(KeyCode::Enter);
        let uses: Vec<UseOn> = stage.app.world_mut().resource_mut::<Messages<Intent<UseOn>>>().drain().map(|i| i.action).collect();
        assert_eq!(uses, vec![UseOn { item: polish, on: boots }], "the use, on the thing picked out");
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
        stage.press(KeyCode::KeyU);
        stage.press(KeyCode::Escape);
        assert!(!stage.app.world().resource::<Modals>().any_open(), "put away");
        assert_eq!((charges(&stage), now(&stage)), (3, before), "no charge, no time");
        assert!(!marked(&stage, coat) && !marked(&stage, boots), "and nothing done to anything");
        assert!(stage.app.world().get::<MyTurn>(stage.player).is_some(), "and the turn is still the player's");

        stage.press(KeyCode::KeyI);
        assert_eq!(inside(&stage, 1), "polish", "the bag opens as it always does, not choosing");
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

    /// A game's own question opens the bag choosing under its prompt among
    /// only what it names that the bag holds; put away it answers nothing,
    /// and confirmed it answers with the thing picked out, puts every
    /// screen away, and spends no time and nothing of the thing, since what
    /// the answer does is the game's.
    #[test]
    fn a_games_question_chooses_among_what_it_names_and_confirm_answers_it_with_the_thing_picked_out() {
        let (mut stage, polish, coat, stone, boots) = chooser();
        let (user, asker, verb) = (stage.player, coat, rl_bevy::Verbs::SEARCH);
        let stray = stage.app.world_mut().spawn((Item, Name::new("stray"))).id();
        let ask = AskForThing { user, asker, verb, prompt: "Hand over which?".into(), choices: vec![stone, stray, polish] };
        let modals = |stage: &Stage| stage.app.world().resource::<Modals>().any_open();
        let answers = |stage: &mut Stage| stage.app.world_mut().resource_mut::<Messages<ThingChosen>>().drain().collect::<Vec<_>>();
        stage.app.world_mut().write_message(ask.clone());
        stage.tick();
        assert!(stage.row(0).contains(" Hand over which? "), "under the game's prompt: {:?}", stage.row(0));
        assert!(inside(&stage, 1) == "stone" && inside(&stage, 2) == "polish", "only what it names that the bag holds: {:?}", stage.rows());
        stage.press(KeyCode::Escape);
        assert!(!modals(&stage) && answers(&mut stage).is_empty(), "put away, it answers nothing");

        let before = stage.app.world().resource::<Turns>().now();
        stage.app.world_mut().write_message(ask);
        stage.tick();
        stage.press(KeyCode::ArrowDown);
        stage.press(KeyCode::Enter);
        assert_eq!(answers(&mut stage), vec![ThingChosen { user, asker, verb, thing: polish }], "the thing picked out");
        assert!(!modals(&stage), "and every screen put away");
        assert_eq!(stage.app.world().resource::<Turns>().now(), before, "no time spent");
        assert_eq!(stage.app.world().get::<Consumable>(polish).expect("charges").left, 3, "and nothing of the thing");
        assert!(stage.app.world().get::<Inventory>(user).expect("a bag").items.contains(&boots), "the bag as it was");
    }

    /// A game's question asked by someone beside the player is answered
    /// while they still stand there offering what asked; once they are
    /// gone out of reach, choosing answers nothing and puts the bag away.
    #[test]
    fn a_games_question_whose_asker_went_out_of_reach_is_answered_nothing() {
        let plugins = (rl_bevy::PropsPlugin, InventoryPanel::new(Rect::new(0, 0, 52, 14)).title("Bag"));
        let mut stage = Stage::new_with(plugins, |app| {
            app.add_verb("give");
        })
        .screen(52, 14);
        let user = stage.player;
        let verb = stage.app.world().resource::<Verbs>().get("give").expect("declared");
        let stone = stage.app.world_mut().spawn((Item, Name::new("stone"))).id();
        stage.app.world_mut().entity_mut(user).insert(Inventory { items: vec![stone] });
        let beside = stage.at.offset(1, 0);
        let asker =
            stage.app.world_mut().spawn((Actor, Position(beside), OnMap(MapId::SURFACE), Name::new("Mirela"), Offers(vec![Offering::new(verb, 100)]))).id();
        stage.tick();
        let answers = |stage: &mut Stage| stage.app.world_mut().resource_mut::<Messages<ThingChosen>>().drain().collect::<Vec<_>>();
        let ask = AskForThing { user, asker, verb, prompt: "Hand over which?".into(), choices: vec![stone] };

        stage.app.world_mut().write_message(ask.clone());
        stage.tick();
        stage.press(KeyCode::Enter);
        assert_eq!(answers(&mut stage), vec![ThingChosen { user, asker, verb, thing: stone }], "answered while beside");

        stage.app.world_mut().write_message(ask);
        stage.tick();
        stage.app.world_mut().entity_mut(asker).insert(Position(stage.at.offset(6, 0)));
        stage.tick();
        stage.press(KeyCode::Enter);
        assert!(answers(&mut stage).is_empty(), "gone out of reach, answered nothing");
        assert!(!stage.app.world().resource::<Modals>().any_open(), "and the bag put away");
    }

    /// A worn thing that will not come off is offered no key that would
    /// take it off or drop it, says so, and those keys do nothing to it.
    #[test]
    fn a_stuck_thing_worn_offers_no_key_that_would_take_it_off_and_says_it_will_not_come_off() {
        let (mut stage, blade, _, _) = staged();
        stage.app.world_mut().entity_mut(blade).insert(Stuck);
        stage.tick();
        stage.press(KeyCode::KeyI);
        let footer = stage.row(13);
        assert!(!footer.contains("take off") && !footer.contains("drop") && footer.contains("esc"), "{footer:?}");
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
        let mut stage = Stage::new_with(InventoryPanel::new(Rect::new(0, 0, 52, 14)).title("Bag"), |app| {
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
            (move |mut view: ResMut<InventoryView>, mut facets: ResMut<crate::Facets>| {
                for row in view.rows.iter_mut().filter(|r| r.entity != silent) {
                    row.seems.push(facets.facet("seems", "worth not yet known").toned(Tones::MUTED));
                    row.seems.push(facets.facet("seems", "known in 3 turns").toned(Tones::BAD));
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
        assert_eq!(inside(&stage, 3), "3 knives", "by its name and how many");
        let said: Vec<String> = detail(&stage)[4..].iter().filter(|l| !l.is_empty()).cloned().collect();
        assert_eq!(said, ["unknown"], "not how far it flies, nor what it strikes for");
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
        let nearly = Consumable { left: 0, recharge: Some(rl_bevy::Recharge { every: 4000, progress: 3950 }), ..Consumable::new(1, WhenEmpty::Kept) };
        let wand = stage.app.world_mut().spawn((Item, Name::new("wand"), triggers, nearly)).id();
        stage.app.world_mut().entity_mut(player).insert(Inventory { items: vec![wand] });
        stage.tick();

        stage.press(KeyCode::KeyI);
        assert!(detail(&stage).iter().any(|l| l == "ready in 1 turn"), "{:?}", detail(&stage));
    }
}
