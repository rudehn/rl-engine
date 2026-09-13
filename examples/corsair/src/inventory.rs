//! The inventory screen: a list menu over the player's bag.
//!
//! The engine's menu widget holds the cursor and draws the frame; this
//! module fills the rows from the bag each frame it is open, maps keys to
//! item actions, and writes the [`Intent`] the engine resolves.

use bevy::prelude::*;
use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_core::Rect;
use rl_engine::rl_render::Terminal;
use rl_engine::rl_ui::{ListMenu, MenuRow, ModalId, Modals, Palette, Tones, draw_menu};

use crate::items::{Armory, ItemKind};
use crate::monsters::Bestiary;

/// The name the sea chest's modal is declared under.
pub const MODAL: &str = "chest";

/// The screen's state. Whether it is open lives on the engine's
/// [`Modals`] stack, so nothing else can take the keys while it is up.
#[derive(Resource)]
pub struct InventoryScreen {
    pub menu: ListMenu,
}

impl Default for InventoryScreen {
    fn default() -> Self {
        let mut menu = ListMenu::new("Sea chest");
        menu.hints = "[e]quip/remove  [d]rop  [u]se  [esc]".into();
        menu.empty = "Nothing but lint.".into();
        Self { menu }
    }
}

/// The id of the chest's modal.
pub fn modal(modals: &Modals) -> ModalId {
    modals.get(MODAL).expect("main declares the chest modal")
}

/// The player, while it holds the turn.
type PlayerTurn<'w, 's> = Query<'w, 's, Entity, (With<Player>, With<MyTurn>)>;

/// The bag, worn or not, as the screen lists it.
type Bag<'w, 's> = Query<'w, 's, (&'static Inventory, &'static Equipped), With<Player>>;

/// Opens, closes and drives the screen; item actions become intents when
/// the player holds the turn, and close the screen.
/// What the sea chest can ask for.
#[derive(bevy::ecs::system::SystemParam)]
pub struct ChestIntents<'w> {
    equips: MessageWriter<'w, Intent<Equip>>,
    unequips: MessageWriter<'w, Intent<Unequip>>,
    drops: MessageWriter<'w, Intent<DropItem>>,
    uses: MessageWriter<'w, Intent<UseItem>>,
}

pub fn inventory_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut screen: ResMut<InventoryScreen>,
    mut modals: ResMut<Modals>,
    player: PlayerTurn,
    bag: Bag,
    mut intents: ChestIntents,
) {
    let chest = modal(&modals);
    if keys.just_pressed(KeyCode::KeyI) && (modals.is_top(chest) || !modals.any_open()) {
        modals.toggle(chest);
        return;
    }
    if !modals.is_top(chest) {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        modals.close_one(chest);
        return;
    }
    if keys.any_just_pressed([KeyCode::ArrowDown, KeyCode::KeyJ]) {
        screen.menu.move_by(1);
    }
    if keys.any_just_pressed([KeyCode::ArrowUp, KeyCode::KeyK]) {
        screen.menu.move_by(-1);
    }
    let Ok(entity) = player.single() else { return };
    let Ok((inventory, worn)) = bag.single() else { return };
    let Some(item) = inventory.items.get(screen.menu.selected).copied() else { return };
    let asked = if keys.just_pressed(KeyCode::KeyE) {
        if worn.contains(item) {
            intents.unequips.write(Intent::new(entity, Unequip(item)));
        } else {
            intents.equips.write(Intent::new(entity, Equip(item)));
        }
        true
    } else if keys.just_pressed(KeyCode::KeyD) {
        intents.drops.write(Intent::new(entity, DropItem(item)));
        true
    } else if keys.any_just_pressed([KeyCode::KeyU, KeyCode::Enter]) {
        intents.uses.write(Intent::new(entity, UseItem(item)));
        true
    } else {
        false
    };
    if asked {
        // An action spends a turn, and the turn loop assumes no screen is
        // up when it runs.
        modals.close_all();
    }
}

/// The tables the chest reads an item's line from.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Ledger<'w, 's> {
    armory: Res<'w, Armory>,
    bestiary: Res<'w, Bestiary>,
    bag: Bag<'w, 's>,
    items: Query<'w, 's, (&'static ItemKind, Option<&'static Stack>, Option<&'static Enchant>)>,
}

/// Fills the rows from the bag and draws the screen over the map.
pub fn draw_inventory(mut screen: ResMut<InventoryScreen>, modals: Res<Modals>, palette: Res<Palette>, mut terminal: ResMut<Terminal>, world: Ledger) {
    if !modals.is_open(modal(&modals)) {
        return;
    }
    let Ledger { armory, bestiary, bag, items } = &world;
    let Ok((inventory, worn)) = bag.single() else { return };
    let rows: Vec<MenuRow> = inventory
        .items
        .iter()
        .filter_map(|&item| {
            let (kind, stack, enchant) = items.get(item).ok()?;
            let d = armory.defs.get(kind.0);
            let mut detail = Vec::new();
            if let (Some(dice), Some(kind_name)) = (&d.attack, &d.kind) {
                let dice = enchant.map(|e| e.0.strike(*dice, &armory.rule(kind.0))).unwrap_or(*dice);
                detail.push(format!("{dice} {kind_name}"));
            }
            if let Some((range, dice, kind_name)) = &d.ranged {
                detail.push(format!("shoots {dice} {kind_name} to {range}"));
            }
            if let Some(e) = enchant {
                for (k, dice) in e.0.strikes(&armory.affixes) {
                    detail.push(format!("+{dice} {}", bestiary.kinds.name(k)));
                }
                for m in e.0.modifiers(&armory.affixes, &armory.rule(kind.0), 0) {
                    if let rl_engine::rl_rules::Op::Add(n) = m.op {
                        detail.push(format!("{} {n:+}", armory.stats.name(m.stat)));
                    }
                }
            }
            if d.armor != 0 {
                detail.push(format!("armor {:+}", d.armor));
            }
            if d.heal > 0 {
                detail.push(format!("restores {} health when drunk", d.heal));
            }
            if let Some(s) = &d.slot {
                detail.push(format!("worn on the {s}"));
            }
            let label = match stack {
                Some(s) if s.count > 1 => format!("{} {}", s.count, d.name),
                _ => armory.display_name(kind.0, enchant),
            };
            let tag = match worn.slot_of(item) {
                Some(slot) => format!("({})", armory.slots.name(slot)),
                None => String::new(),
            };
            let category = if worn.contains(item) { Tones::NOTICE } else { Tones::TEXT };
            Some(MenuRow::new(label).tag(tag).detail(detail.join(", ")).toned(category))
        })
        .collect();
    screen.menu.set_rows(rows);
    let bounds = terminal.bounds();
    // Tall enough for the rows, the detail line and the frame; never taller than the screen.
    let wanted = screen.menu.rows.len().max(3) as i32 + 5;
    let (w, h) = (bounds.width.min(56), bounds.height.min(22).min(wanted));
    let rect = Rect::new((bounds.width - w) / 2, (bounds.height - h) / 2, w, h);
    draw_menu(&mut terminal, rect, &screen.menu, &palette);
}
