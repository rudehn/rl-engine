//! The bag: what you carry, what each thing is worth, and what to do with
//! it.
//!
//! A modal the engine runs end to end, the way the ability menu is:
//! [`InventoryKeys::toggle`] opens and closes it, the direction keys walk
//! the rows, and four keys act on the row picked out. Wearing, dropping and
//! using are the engine's own item actions, so the screen writes their
//! intents itself; throwing opens the targeting cursor through
//! [`AimThrow`], the way a game's throw key would. A game binds nothing.
//! Using an item that does something itself uses it where it stands, and
//! using one that lends an ability uses the ability: on the spot when
//! it needs no aim, through the targeting cursor by [`AimAt`] when it does.
//! Nothing else is used from here; an item a game answers itself from
//! [`ItemEvent::Used`] is used by the game's own key, since the bag cannot
//! know it does anything and a use that did nothing would spend a turn.
//! The footer offers only the keys that do something to the row picked
//! out: wear or take off, drop, use, throw, and close.
//!
//! Under the rows, the row picked out is described from its own
//! components: the blow it is swung with, the shot it fires, what it adds
//! to armor or a stat, how far it flies, where it is worn or could be, what
//! it lends and how many uses are left, and whatever the game pushed onto
//! it in [`ViewSet::Annotate`](crate::ViewSet).
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
use crate::panel::sheet::plain_op;
use crate::panel::{clear, clip, frame, wrap};
use crate::tone::{Palette, ToneId, Tones};
use crate::view::inventory::{InventoryView, InventoryViewPlugin, ItemRow};
use crate::view::sheet::Strike;
use crate::view::target::{AimAt, AimThrow};

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

/// Which row the cursor is on. The screen's own state, since a view holds
/// no cursor.
#[derive(Resource, Debug, Default)]
pub struct InventoryMenu {
    /// The row picked out, clamped to the rows there are when drawn.
    pub selected: usize,
}

impl InventoryMenu {
    /// Moves the cursor `by` rows over `rows` of them, wrapping at either
    /// end.
    pub fn move_by(&mut self, by: i32, rows: usize) {
        if rows == 0 {
            self.selected = 0;
            return;
        }
        self.selected = (self.selected as i64 + by as i64).rem_euclid(rows as i64) as usize;
    }
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
            .add_message::<AimThrow>()
            .add_message::<AimAt>();
        app.insert_resource(self.0.clone()).init_resource::<InventoryMenu>().init_resource::<InventoryKeys>();
        app.add_systems(Update, inventory_keys.in_set(EngineSet::Input)).add_systems(Update, draw_inventory.in_set(PresentSet::Overlay));
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

/// What the bag can ask for.
#[derive(bevy::ecs::system::SystemParam)]
pub struct BagIntents<'w> {
    equips: MessageWriter<'w, Intent<Equip>>,
    unequips: MessageWriter<'w, Intent<Unequip>>,
    drops: MessageWriter<'w, Intent<DropItem>>,
    uses: MessageWriter<'w, Intent<UseItem>>,
    aims: MessageWriter<'w, AimAt>,
    throws: MessageWriter<'w, AimThrow>,
}

/// Opens, closes and walks the bag, and acts on the row picked out.
///
/// An action is taken only while the player holds the turn, since an intent
/// written for anyone else is dropped unread; the screen stays up until it
/// does.
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
    if binds.toggle.just_pressed(keys.input()) && (modals.is_top(modal) || !modals.any_open()) {
        modals.toggle(modal);
        return;
    }
    if !modals.is_top(modal) {
        return;
    }
    let bindings = keys.bindings();
    let input = keys.input();
    if input.just_pressed(bindings.cursor.close) {
        modals.close_one(modal);
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
    let acted = if binds.wear.just_pressed(input) {
        if row.worn() {
            intents.unequips.write(Intent::new(user, Unequip(item)));
            true
        } else if row.wearable() {
            intents.equips.write(Intent::new(user, Equip(item)));
            true
        } else {
            false
        }
    } else if binds.drop.just_pressed(input) {
        intents.drops.write(Intent::new(user, DropItem(item)));
        true
    } else if binds.use_it.just_pressed(input) || input.just_pressed(bindings.cursor.confirm) || input.just_pressed(bindings.cursor.also_confirm) {
        // What lends an ability, or what does something itself. Anything
        // else is not used from here: it would spend a turn on nothing, and
        // the footer never offers it.
        match (row.lends.first(), row.usable()) {
            // An aimed ability wants the cursor; the bag closes for it.
            (Some(lent), _) if lent.aimed => {
                intents.aims.write(AimAt { user, ability: lent.ability });
                true
            }
            (Some(_), _) | (None, true) => {
                intents.uses.write(Intent::new(user, UseItem(item)));
                true
            }
            (None, false) => false,
        }
    } else if binds.throw.just_pressed(input) && row.throw_range.is_some() {
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
/// Only what applies, so the footer never offers to wear a pebble. Short
/// enough for a narrow frame: the direction keys walk the rows on every
/// screen and need no saying here.
fn hints(row: Option<&ItemRow>, binds: &InventoryKeys, confirm: String, close: String) -> String {
    let mut keys = Vec::new();
    if let Some(row) = row {
        if row.worn() {
            keys.push(format!("{} take off", binds.wear.label()));
        } else if row.wearable() {
            keys.push(format!("{} wear", binds.wear.label()));
        }
        keys.push(format!("{} drop", binds.drop.label()));
        if !row.lends.is_empty() || row.usable() {
            keys.push(format!("{confirm} use"));
        }
        if row.throw_range.is_some() {
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

/// The lines describing `row`: its numbers, then where it goes, then what
/// the game added.
fn describe(row: &ItemRow, width: usize) -> Vec<(String, ToneId)> {
    let mut lines: Vec<(String, ToneId)> = Vec::new();
    let mut say = |text: String, tone: ToneId| lines.extend(wrap(&text, width).into_iter().map(|l| (l, tone)));
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
    for (stat, op) in &row.bestows {
        say(format!("{stat} {}", plain_op(*op)), Tones::TEXT);
    }
    match (&row.thrown, row.throw_range) {
        (Some(thrown), _) => say(format!("thrown {}", strike(thrown)), Tones::TEXT),
        (None, Some(range)) => say(format!("thrown to {range}"), Tones::TEXT),
        (None, None) => {}
    }
    if row.worn() {
        say(format!("worn on the {}", row.slot_name), Tones::MUTED);
    } else if row.wearable() {
        say(format!("goes on the {}", row.goes_on.join(" or the ")), Tones::MUTED);
    }
    for what in &row.used {
        say(format!("use: {what}"), Tones::TEXT);
    }
    for lent in &row.lends {
        say(format!("use: {}", lent.name), Tones::TEXT);
        if !lent.description.is_empty() {
            say(lent.description.clone(), Tones::MUTED);
        }
        match lent.charges {
            Some(1) => say("1 charge left".to_string(), Tones::MUTED),
            Some(n) => say(format!("{n} charges left"), Tones::MUTED),
            None => {}
        }
    }
    for facet in &row.facets {
        say(facet.text.clone(), facet.tone);
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
/// picked out described under it.
pub fn draw_inventory(mut terminal: ResMut<Terminal>, mut menu: ResMut<InventoryMenu>, screen: BagScreen) {
    let BagScreen { layout, view, binds, keys, modals, palette } = &screen;
    if !modals.is_open(inventory_modal(modals)) {
        return;
    }
    let rect = layout.rect;
    if rect.width < 12 || rect.height < 6 {
        return;
    }
    let bindings = keys.bindings();
    // The cursor survives the rebuild, clamped to the rows there are now,
    // before the footer asks which row it is on.
    menu.selected = menu.selected.min(view.rows.len().saturating_sub(1));
    let hints = hints(view.row(menu.selected), binds, key_name(bindings.cursor.confirm), key_name(bindings.cursor.close));
    clear(&mut terminal, rect, palette);
    frame(&mut terminal, rect, &layout.title, &hints, palette);
    let inner = Rect::new(rect.x + 2, rect.y + 1, rect.width - 4, rect.height - 2);
    let surface = palette.get(Tones::SURFACE);
    let width = inner.width.max(0) as usize;

    if view.rows.is_empty() {
        terminal.print_on(inner.x, inner.y, &clip(&layout.empty, width), palette.get(Tones::MUTED), surface);
        return;
    }
    // Half the height to the rows, the rest to the words about the one
    // picked out, and never fewer than three rows.
    let list_rows = ((inner.height / 2) as usize).max(3).min(view.rows.len());
    let first = menu.selected.saturating_sub(list_rows - 1).min(view.rows.len().saturating_sub(list_rows));
    for (i, row) in view.rows.iter().enumerate().skip(first).take(list_rows) {
        let y = inner.y + (i - first) as i32;
        let selected = i == menu.selected;
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
    let Some(row) = view.row(menu.selected) else { return };
    for (y, (line, tone)) in (rule_y + 1..inner.bottom()).zip(describe(row, width)) {
        terminal.print_on(inner.x, y, &clip(&line, width), palette.get(tone), surface);
    }
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
        let hat = stage.app.world_mut().spawn((Item, Name::new("a hat"), Wearable(EquipShape::in_slot(head)), Armor(1))).id();
        let knives = stage
            .app
            .world_mut()
            .spawn((Item, Name::new("knife"), Stack { key: 1, count: 3 }, Throwable { range: 5, strike: Some((kind, DiceRoll::new(1, 4))) }))
            .id();
        let mut worn = Equipped(Equipment::with_slot_count(2));
        worn.equip(blade, &EquipShape::in_slot(hand)).unwrap();
        stage.app.world_mut().entity_mut(player).insert((Inventory { items: vec![blade, hat, knives] }, worn));
        stage.tick();
        (stage, blade, hat, knives)
    }

    /// The text inside the frame on row `y`, border and padding cut off.
    fn inside(stage: &Stage, y: i32) -> String {
        stage.row(y).trim_start_matches("\u{2502} ").trim_end_matches('\u{2502}').trim_end().to_string()
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
        assert_eq!(inside(&stage, 6), "goes on the head");
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

    #[test]
    fn an_empty_bag_says_so_in_the_games_words() {
        let (mut stage, _, _, _) = staged();
        let player = stage.player;
        stage.app.world_mut().entity_mut(player).insert(Inventory::default());
        stage.press(KeyCode::KeyI);
        assert_eq!(inside(&stage, 1), "Lint.");
    }

    /// An item that lends an ability is described in the ability's words,
    /// and using it uses the ability: on the spot for one that needs no
    /// aim, through the targeting cursor for one that does.
    #[test]
    fn an_item_that_lends_an_ability_is_used_through_it_and_an_aimed_one_opens_the_cursor() {
        let mut stage = Stage::new_with((ThrowingPlugin, crate::TargetViewPlugin, InventoryPanel::new(Rect::new(0, 0, 52, 14)).title("Bag")), |app| {
            app.add_engine_effects();
            let names = Names::new().damage_kinds(&app.world().resource::<Registries>().damage_kinds);
            let kinds = app.world().resource::<EffectKinds>();
            let text = r#"#![enable(implicit_some)]
[
    (name: "quaff", description: "A swallow. It closes a wound.", aim: SelfOnly, mode: Own, costs: [Charge(1)],
     effects: [(kind: "Mend", args: (kind: "kinetic", roll: "5"))]),
    (name: "zap", mode: Bolt(range: 6), costs: [Charge(1)], effects: [(kind: "Harm", args: (kind: "kinetic", roll: "3"))]),
]"#;
            let abilities = Abilities::load(text, kinds, &names).expect("the abilities load");
            app.insert_resource(abilities);
        })
        .screen(52, 14);
        let player = stage.player;
        let (quaff, zap) = {
            let a = stage.app.world().resource::<Abilities>();
            (a.expect("quaff"), a.expect("zap"))
        };
        let potions = stage.app.world_mut().spawn((Item, Name::new("potion"), Grants(vec![quaff]), Stack { key: 1, count: 2 })).id();
        let wand = stage.app.world_mut().spawn((Item, Name::new("a wand"), Grants(vec![zap]), Charges::full(3))).id();
        stage.app.world_mut().entity_mut(player).insert(Inventory { items: vec![potions, wand] });
        stage.tick();

        stage.press(KeyCode::KeyI);
        assert_eq!(inside(&stage, 1), "2 potions");
        assert_eq!(inside(&stage, 4), "use: quaff", "described as what it lends");
        assert_eq!(inside(&stage, 5), "A swallow. It closes a wound.", "in the game's words");
        stage.press(KeyCode::ArrowDown);
        assert_eq!(inside(&stage, 4), "use: zap");
        assert_eq!(inside(&stage, 5), "3 charges left", "and counted when it counts");

        stage.press(KeyCode::Enter);
        let aims: Vec<AimAt> = stage.app.world_mut().resource_mut::<Messages<AimAt>>().drain().collect();
        assert_eq!(aims, vec![AimAt { user: player, ability: zap }], "an aimed ability opens the cursor");
        assert!(stage.app.world_mut().resource_mut::<Messages<Intent<UseItem>>>().drain().next().is_none(), "rather than using the wand on the spot");

        // The bag closed for the cursor; Escape puts the cursor away too.
        stage.press(KeyCode::Escape);
        assert!(!stage.app.world().resource::<Modals>().any_open());
        stage.press(KeyCode::KeyI);
        stage.press(KeyCode::ArrowUp);
        stage.press(KeyCode::KeyU);
        let uses: Vec<Entity> = stage.app.world_mut().resource_mut::<Messages<Intent<UseItem>>>().drain().map(|i| i.action.0).collect();
        assert_eq!(uses, vec![potions], "one that needs no aim is used where the player stands");
    }

    /// A thing that does something itself is offered and described from its
    /// own effects, with no ability anywhere: no registry, no `Known`, and
    /// the same `use:` line the ability path writes, because both ask the
    /// same list what it does.
    #[test]
    fn a_thing_that_does_something_itself_is_offered_and_described_from_its_own_effects() {
        let mut stage = Stage::new_with(InventoryPanel::new(Rect::new(0, 0, 52, 14)).title("Bag"), |app| {
            app.add_engine_effects();
        })
        .screen(52, 14);
        let player = stage.player;
        let effects = {
            let specs = vec![rl_rules::EffectSpec {
                kind: "Mend".into(),
                chance: 100,
                args: rl_rules::ability::parse_args(r#"(kind: "kinetic", roll: "5")"#).expect("the args parse"),
            }];
            let registries = stage.app.world().resource::<Registries>().clone();
            let kinds = stage.app.world().resource::<EffectKinds>();
            rl_bevy::Effects::build(&specs, kinds, &registries.names()).expect("the mend builds")
        };
        let poultice =
            stage.app.world_mut().spawn((Item, Name::new("poultice"), OnUse(std::sync::Arc::new(effects)), Consumable, Stack { key: 1, count: 2 })).id();
        stage.app.world_mut().entity_mut(player).insert(Inventory { items: vec![poultice] });
        stage.tick();

        stage.press(KeyCode::KeyI);
        assert_eq!(inside(&stage, 1), "2 poultices");
        assert_eq!(inside(&stage, 3), "use: mends 5 kinetic", "described by what it does, in the registries' names");
        assert!(stage.app.world().resource::<InventoryView>().rows[0].usable(), "and the row knows it is used");

        stage.press(KeyCode::KeyU);
        let uses: Vec<Entity> = stage.app.world_mut().resource_mut::<Messages<Intent<UseItem>>>().drain().map(|i| i.action.0).collect();
        assert_eq!(uses, vec![poultice], "and the use key uses it, with no ability to lend it");
    }
}
