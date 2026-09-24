//! The cheat menu, for trying a thing without finding it first and a deck
//! without walking the decks above it.
//!
//! `\` opens it over a run. Each row is a key as well as a row: `r` reveals
//! the deck, `h` heals, `g` is godmode, `n` and `p` take the lift a deck
//! down or up, and `s` searches the armory by name and puts what is picked
//! in the pack. Two of them are toggles held in [`Cheats`] and kept true by
//! a system each rather than done once, so they outlast a deck change and a
//! new run: [`keep_godmode`] keeps the engine's
//! [`Invulnerable`] on the commando, and
//! [`reveal_the_deck`] marks every deck the commando stands on as seen.
//!
//! Nothing here reaches past the engine's own doors. Godmode is the
//! engine's marker, so a hit still lands and says it had no effect; a deck
//! change is the `WarpRequest` a lift writes, so the deck fills on arrival
//! the way it always does; and an item comes from the same `spawn_item` a
//! crate's does, merged into a stack the way a pickup merges it and
//! reported as one, so whatever reacts to a pickup reacts to this too.
//!
//! A screen, so the engine's modal stack is what gives it the keyboard:
//! the world's keys stand down while it is up, and `esc` puts away the top
//! screen, which is how the search steps back to the menu.

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_core::{Id, Rect};

use crate::decks::{DECKS, deck_of, map_of};
use crate::gear::{Content, ItemDef, spawn_item};
use crate::input::Binds;

/// The name the menu's modal is declared under.
pub const MENU: &str = "cheats";
/// The name the item search's modal is declared under.
pub const SEARCH: &str = "cheat search";

/// The two cheats that stay on until they are turned off.
///
/// Kept for the whole process rather than reset with a run, so a player
/// testing the late decks keeps godmode through a death's restart.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Cheats {
    /// The commando takes no harm.
    pub godmode: bool,
    /// Every deck the commando is on is drawn as though walked.
    pub reveal: bool,
}

/// One row of the menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cheat {
    /// Toggles [`Cheats::reveal`].
    RevealMap,
    /// Health back to full.
    Heal,
    /// Toggles [`Cheats::godmode`].
    Godmode,
    /// The lift down, from wherever the commando stands.
    NextDeck,
    /// The lift up.
    PreviousDeck,
    /// Opens the item search.
    AddItem,
}

impl Cheat {
    /// Every row, in the order they are drawn.
    pub const ALL: [Cheat; 6] = [Cheat::RevealMap, Cheat::Heal, Cheat::Godmode, Cheat::NextDeck, Cheat::PreviousDeck, Cheat::AddItem];

    /// The key that takes the row without moving the cursor to it.
    fn key(self) -> KeyCode {
        match self {
            Cheat::RevealMap => KeyCode::KeyR,
            Cheat::Heal => KeyCode::KeyH,
            Cheat::Godmode => KeyCode::KeyG,
            Cheat::NextDeck => KeyCode::KeyN,
            Cheat::PreviousDeck => KeyCode::KeyP,
            Cheat::AddItem => KeyCode::KeyS,
        }
    }

    /// The row as the menu shows it: its key, what it is, and what it does.
    fn row(self, cheats: &Cheats) -> MenuRow {
        let (key, label, detail) = match self {
            Cheat::RevealMap => ('r', "Reveal map", "Every deck drawn as though walked."),
            Cheat::Heal => ('h', "Heal", "Health back to full."),
            Cheat::Godmode => ('g', "Godmode", "Every hit lands as nothing."),
            Cheat::NextDeck => ('n', "Next deck", "The lift down, from here."),
            Cheat::PreviousDeck => ('p', "Previous deck", "The lift up, from here."),
            Cheat::AddItem => ('s', "Search items", "Find anything by name, and take it."),
        };
        let on = match self {
            Cheat::RevealMap => Some(cheats.reveal),
            Cheat::Godmode => Some(cheats.godmode),
            _ => None,
        };
        let row = MenuRow::new(format!("[{key}] {label}")).detail(detail);
        match on {
            Some(true) => row.tag("on").toned(Tones::GOOD),
            Some(false) => row.tag("off"),
            None => row,
        }
    }
}

/// What the two screens show: the menu, and the search with what has been
/// typed and what it matches.
#[derive(Resource, Default)]
pub struct CheatScreen {
    menu: ListMenu,
    search: ListMenu,
    query: String,
    matches: Vec<Id<ItemDef>>,
}

impl CheatScreen {
    /// Fills the menu's rows from `cheats`, keeping the cursor where it is.
    fn refresh(&mut self, cheats: &Cheats) {
        self.menu.title = "Cheats".to_string();
        self.menu.hints = "\u{2191}\u{2193} pick \u{2022} enter or a key \u{2022} esc close".to_string();
        self.menu.set_rows(Cheat::ALL.iter().map(|c| c.row(cheats)).collect());
    }

    /// Every item whose name holds what is typed, ignoring case, in
    /// alphabetical order: the whole armory while nothing is typed.
    fn filter(&mut self, content: &Content) {
        let armory = content.armory();
        let query = self.query.to_lowercase();
        let mut found: Vec<(Id<ItemDef>, String)> =
            armory.defs.iter().filter(|(_, d)| d.name.to_lowercase().contains(&query)).map(|(id, d)| (id, d.name.clone())).collect();
        found.sort_by(|a, b| a.1.cmp(&b.1));
        self.search.title = format!("Add to pack: {}_", self.query);
        self.search.hints = "type to search \u{2022} \u{2191}\u{2193} pick \u{2022} enter take \u{2022} esc back".to_string();
        self.search.empty = "Nothing by that name.".to_string();
        self.search.set_rows(found.iter().map(|(_, name)| MenuRow::new(name.clone())).collect());
        self.matches = found.into_iter().map(|(id, _)| id).collect();
    }
}

/// The id of the modal `name` names.
fn modal(modals: &Modals, name: &str) -> ModalId {
    modals.get(name).expect("CheatsPlugin declares both of its screens")
}

/// The menu's two screens and what they show.
///
/// The keys and the two toggles' upkeep are ordered by
/// [`FoundryPlugin`](crate::plugin::FoundryPlugin) with the rest of
/// Foundry's keys: the keys ahead of the world's own, so a key taken by a
/// cheat screen is never also a step, and the upkeep after them all, so a
/// toggle takes hold the frame it is pressed.
pub struct CheatsPlugin;

impl Plugin for CheatsPlugin {
    fn build(&self, app: &mut App) {
        app.add_modal(MENU).add_modal(SEARCH);
        app.init_resource::<Cheats>().init_resource::<CheatScreen>();
    }
}

/// Everything a cheat can change about the run.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Acts<'w, 's> {
    commands: Commands<'w, 's>,
    cheats: ResMut<'w, Cheats>,
    log: ResMut<'w, MessageLog>,
    turns: Res<'w, Turns>,
    map: Res<'w, WorldMap>,
    warps: MessageWriter<'w, WarpRequest>,
    picked_up: MessageWriter<'w, ItemEvent>,
    player: Query<'w, 's, (Entity, &'static mut Health, &'static mut Inventory), With<Player>>,
    stacks: Query<'w, 's, &'static mut Stack>,
    content: Content<'w>,
}

impl Acts<'_, '_> {
    /// Says `text` in the log, muted, since a cheat is not the run's news.
    fn say(&mut self, text: impl Into<String>) {
        let now = self.turns.turn_number();
        self.log.muted(text, now);
    }

    /// Does `cheat`, and says whether it took the commando off the deck,
    /// which closes the menu behind it.
    fn run(&mut self, cheat: Cheat) -> bool {
        let Ok((me, mut health, _)) = self.player.single_mut() else { return false };
        let deck = deck_of(self.map.current());
        match cheat {
            Cheat::RevealMap => {
                self.cheats.reveal = !self.cheats.reveal;
                let now = if self.cheats.reveal { "on" } else { "off" };
                self.say(format!("Reveal map {now}."));
            }
            Cheat::Heal => {
                health.current = health.max;
                self.say("Healed to full.");
            }
            Cheat::Godmode => {
                self.cheats.godmode = !self.cheats.godmode;
                let now = if self.cheats.godmode { "on" } else { "off" };
                self.say(format!("Godmode {now}."));
            }
            Cheat::NextDeck if deck < DECKS => {
                self.warps.write(WarpRequest { actor: me, to: Destination::Place { map: map_of(deck + 1), arrive: Arrive::Entry } });
                return true;
            }
            Cheat::NextDeck => self.say("There is no deck below the core."),
            Cheat::PreviousDeck if deck > 1 => {
                self.warps.write(WarpRequest { actor: me, to: Destination::Place { map: map_of(deck - 1), arrive: Arrive::Exit } });
                return true;
            }
            Cheat::PreviousDeck => self.say("There is no deck above the first."),
            Cheat::AddItem => {}
        }
        false
    }

    /// Puts one `id` in the commando's pack, merged into a stack of it if
    /// there is one, the way a pickup merges it.
    fn give(&mut self, id: Id<ItemDef>) {
        let armory = self.content.armory();
        let item = spawn_item(&mut self.commands, &armory, id, self.content.registries());
        let Ok((me, _, mut bag)) = self.player.single_mut() else { return };
        let stacking = armory.defs.get(id).stack;
        let key = id.index() as u64;
        let into = if stacking { bag.items.iter().copied().find(|c| self.stacks.get(*c).is_ok_and(|s| s.key == key)) } else { None };
        match into {
            Some(into) => {
                if let Ok(mut stack) = self.stacks.get_mut(into) {
                    stack.count += 1;
                }
                self.commands.entity(item).despawn();
            }
            None => bag.items.push(item),
        }
        // The narrator's line for it, as for any pickup; a line of the
        // menu's own as well said everything twice.
        self.picked_up.write(ItemEvent::PickedUp { actor: me, item, merged_into: into });
    }
}

/// Opens the menu on `\`, while nothing else is on screen.
///
/// After [`menu_keys`], which closes the menu on the same key: the frame a
/// screen closes on still counts as one being up, so the key that closed
/// it does not open it again here.
pub fn open_cheats(keys: ControlInput, binds: Res<Binds>, mut modals: ResMut<Modals>, mut screen: ResMut<CheatScreen>, cheats: Res<Cheats>) {
    if modals.any_open() || !keys.just_pressed(binds.cheats) {
        return;
    }
    screen.refresh(&cheats);
    let id = modal(&modals, MENU);
    modals.open(id);
}

/// The menu's keys: the arrows and enter, or a row's own letter; `\`
/// closes it again.
///
/// The arrows alone move the cursor, not every direction key: `h` is a
/// step west to the world and the heal here.
pub fn menu_keys(
    input: Res<ButtonInput<KeyCode>>,
    keys: ControlInput,
    binds: Res<Binds>,
    mut modals: ResMut<Modals>,
    mut screen: ResMut<CheatScreen>,
    mut acts: Acts,
) {
    let menu = modal(&modals, MENU);
    if !modals.is_top(menu) {
        return;
    }
    if keys.just_pressed(binds.cheats) {
        modals.close_one(menu);
        return;
    }
    if input.just_pressed(KeyCode::ArrowUp) {
        screen.menu.move_by(-1);
    }
    if input.just_pressed(KeyCode::ArrowDown) {
        screen.menu.move_by(1);
    }
    let by_letter = Cheat::ALL.into_iter().find(|c| input.just_pressed(c.key()));
    let by_cursor = (input.just_pressed(KeyCode::Enter) || input.just_pressed(KeyCode::NumpadEnter)).then(|| Cheat::ALL[screen.menu.selected]);
    let Some(cheat) = by_letter.or(by_cursor) else { return };
    if let Some(i) = Cheat::ALL.iter().position(|c| *c == cheat) {
        screen.menu.selected = i;
    }
    if cheat == Cheat::AddItem {
        screen.query.clear();
        screen.filter(&acts.content);
        let search = modal(&modals, SEARCH);
        modals.open(search);
        return;
    }
    if acts.run(cheat) {
        modals.close_one(menu);
    }
    screen.refresh(&acts.cheats);
}

/// The letter, digit or mark `key` types into the search: enough for every
/// name in `items.ron`.
fn typed(key: KeyCode) -> Option<char> {
    use KeyCode::*;
    let letters = [
        KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM, KeyN, KeyO, KeyP, KeyQ, KeyR, KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY,
        KeyZ,
    ];
    let digits = [Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9];
    if let Some(i) = letters.iter().position(|k| *k == key) {
        return Some((b'a' + i as u8) as char);
    }
    if let Some(i) = digits.iter().position(|k| *k == key) {
        return Some((b'0' + i as u8) as char);
    }
    match key {
        Space => Some(' '),
        Minus => Some('-'),
        _ => None,
    }
}

/// The search's keys: what is typed narrows the list, backspace takes a
/// letter off, the arrows pick, and enter puts one of the pick in the
/// pack and leaves the search up for the next.
pub fn search_keys(input: Res<ButtonInput<KeyCode>>, modals: Res<Modals>, mut screen: ResMut<CheatScreen>, mut acts: Acts) {
    if !modals.is_top(modal(&modals, SEARCH)) {
        return;
    }
    let before = screen.query.clone();
    for key in input.get_just_pressed() {
        if let Some(c) = typed(*key) {
            screen.query.push(c);
        }
    }
    if input.just_pressed(KeyCode::Backspace) {
        screen.query.pop();
    }
    if screen.query != before {
        screen.search.selected = 0;
        screen.filter(&acts.content);
    }
    if input.just_pressed(KeyCode::ArrowUp) {
        screen.search.move_by(-1);
    }
    if input.just_pressed(KeyCode::ArrowDown) {
        screen.search.move_by(1);
    }
    let take = input.just_pressed(KeyCode::Enter) || input.just_pressed(KeyCode::NumpadEnter);
    if take && let Some(id) = screen.matches.get(screen.search.selected).copied() {
        acts.give(id);
    }
}

/// Keeps the engine's `Invulnerable` on the commando while godmode is on,
/// and off it while it is off, whichever commando this run has.
pub fn keep_godmode(mut commands: Commands, cheats: Res<Cheats>, players: Query<(Entity, Has<Invulnerable>), With<Player>>) {
    for (me, invulnerable) in &players {
        match (cheats.godmode, invulnerable) {
            (true, false) => {
                commands.entity(me).insert(Invulnerable);
            }
            (false, true) => {
                commands.entity(me).remove::<Invulnerable>();
            }
            _ => {}
        }
    }
}

/// While reveal is on, marks every cell of the deck worth drawing as seen:
/// every floor, and every wall that stands beside one, so the deck reads
/// as a walked deck rather than as a slab of solid hull.
///
/// Reads before it writes, so a deck already revealed is not marked
/// changed every frame.
pub fn reveal_the_deck(cheats: Res<Cheats>, map: Res<WorldMap>, mut knowledge: ResMut<Knowledge>) {
    if !cheats.reveal {
        return;
    }
    let bounds = map.window_tiles();
    for y in bounds.y..bounds.bottom() {
        for x in bounds.x..bounds.right() {
            let p = Point::new(x, y);
            if knowledge.is_explored(p) {
                continue;
            }
            let shows = map.is_walkable(p) || Direction::ALL.iter().any(|d| map.is_walkable(p + d.offset()));
            if shows {
                knowledge.mark(p);
            }
        }
    }
}

/// Draws whichever cheat screen is on top, each in the rectangle it is
/// built with, the way [`ChoicePanel`](crate::upgrades::ChoicePanel) draws
/// the pick: the menu in one cut to fit its six rows, the search in a
/// taller one, since it lists the armory.
#[derive(Resource, Debug, Clone, Copy)]
pub struct CheatPanel {
    /// Where the menu draws.
    pub menu: Rect,
    /// Where the search draws.
    pub search: Rect,
}

impl Plugin for CheatPanel {
    fn build(&self, app: &mut App) {
        app.insert_resource(*self).add_systems(Update, draw_cheats.in_set(PresentSet::Overlay));
    }
}

/// Draws the search over the menu while it is open, and the menu alone
/// otherwise.
fn draw_cheats(screen: Res<CheatScreen>, modals: Res<Modals>, palette: Res<Palette>, rects: Res<CheatPanel>, mut terminal: ResMut<Terminal>) {
    if modals.is_open(modal(&modals, SEARCH)) {
        draw_menu(&mut terminal, rects.search, &screen.search, &palette);
    } else if modals.is_open(modal(&modals, MENU)) {
        draw_menu(&mut terminal, rects.menu, &screen.menu, &palette);
    }
}

#[cfg(test)]
mod tests {
    use rl_engine::rl_bevy::testing::{KeyScriptPlugin, press};
    use rl_engine::rl_core::RunSeed;
    use rl_engine::rl_rules::Hit;

    use super::*;

    /// A run on deck one, with the keys pressed the way a player presses
    /// them, and the commando.
    fn playing() -> (App, Entity) {
        let mut app = crate::testing::headless(RunSeed(4));
        app.add_plugins(KeyScriptPlugin);
        crate::testing::settle(&mut app);
        let me = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
        (app, me)
    }

    fn top(app: &App) -> Option<&str> {
        let modals = app.world().resource::<Modals>();
        modals.top().map(|m| modals.name(m))
    }

    fn deck(app: &App) -> u32 {
        deck_of(app.world().resource::<WorldMap>().current())
    }

    fn keys(app: &mut App, keys: &[KeyCode]) {
        for key in keys {
            press(app, *key);
        }
    }

    #[test]
    fn backslash_opens_the_menu_and_again_or_escape_puts_it_away() {
        let (mut app, _) = playing();
        press(&mut app, KeyCode::Backslash);
        assert_eq!(top(&app), Some(MENU));
        press(&mut app, KeyCode::Backslash);
        assert_eq!(top(&app), None, "the key that opened it closes it");
        press(&mut app, KeyCode::Backslash);
        press(&mut app, KeyCode::Escape);
        assert_eq!(top(&app), None, "and so does escape");
    }

    #[test]
    fn heal_fills_the_commando_back_up() {
        let (mut app, me) = playing();
        app.world_mut().get_mut::<Health>(me).unwrap().current = 3;
        keys(&mut app, &[KeyCode::Backslash, KeyCode::KeyH]);
        let health = app.world().get::<Health>(me).unwrap();
        assert_eq!(health.current, health.max);
        assert_eq!(top(&app), Some(MENU), "and the menu stays up for the next");
    }

    /// Godmode is the engine's own marker: a blow lands and takes nothing,
    /// and turned off, a blow hurts again.
    #[test]
    fn godmode_turns_every_hit_to_nothing_until_it_is_turned_off() {
        let (mut app, me) = playing();
        keys(&mut app, &[KeyCode::Backslash, KeyCode::KeyG]);
        let kinetic = app.world().resource::<Registries>().damage_kinds.expect("kinetic");
        let full = app.world().get::<Health>(me).unwrap().current;
        app.world_mut().write_message(DamageEvent::new(me, Hit::from_source(None, kinetic, 999)));
        app.update();
        assert_eq!(app.world().get::<Health>(me).unwrap().current, full, "nothing taken");
        press(&mut app, KeyCode::KeyG);
        app.world_mut().write_message(DamageEvent::new(me, Hit::from_source(None, kinetic, 5)));
        app.update();
        assert!(app.world().get::<Health>(me).unwrap().current < full, "and off again, it hurts");
    }

    #[test]
    fn next_and_previous_deck_take_the_lift_and_stop_at_the_ends() {
        let (mut app, _) = playing();
        keys(&mut app, &[KeyCode::Backslash, KeyCode::KeyN]);
        crate::testing::settle(&mut app);
        assert_eq!(deck(&app), 2, "down one");
        assert_eq!(top(&app), None, "and the menu is put away on arrival");
        keys(&mut app, &[KeyCode::Backslash, KeyCode::KeyP]);
        crate::testing::settle(&mut app);
        assert_eq!(deck(&app), 1, "and back up");
        keys(&mut app, &[KeyCode::Backslash, KeyCode::KeyP]);
        crate::testing::settle(&mut app);
        assert_eq!(deck(&app), 1, "nothing above the first");
        let said = app.world().resource::<MessageLog>().iter().any(|e| e.text == "There is no deck above the first.");
        assert!(said, "and it says so");
    }

    /// Revealed, every floor of the deck is known, whether or not the
    /// commando has been anywhere near it.
    #[test]
    fn reveal_map_marks_every_floor_of_the_deck_seen() {
        let (mut app, _) = playing();
        keys(&mut app, &[KeyCode::Backslash, KeyCode::KeyR]);
        app.update();
        let map = app.world().resource::<WorldMap>();
        let knowledge = app.world().resource::<Knowledge>();
        let bounds = map.window_tiles();
        let floors: Vec<Point> =
            (bounds.y..bounds.bottom()).flat_map(|y| (bounds.x..bounds.right()).map(move |x| Point::new(x, y))).filter(|p| map.is_walkable(*p)).collect();
        assert!(!floors.is_empty());
        assert!(floors.iter().all(|p| knowledge.is_explored(*p)), "every floor seen");
    }

    /// The whole reason for the menu: part of a name, enter, and it is in
    /// the pack; again, and the stack grows rather than a second one
    /// appearing.
    #[test]
    fn part_of_a_name_and_enter_puts_that_item_in_the_pack_and_a_second_stacks() {
        let (mut app, me) = playing();
        keys(&mut app, &[KeyCode::Backslash, KeyCode::KeyS]);
        assert_eq!(top(&app), Some(SEARCH));
        keys(&mut app, &[KeyCode::KeyF, KeyCode::KeyR, KeyCode::KeyA, KeyCode::KeyG, KeyCode::Enter, KeyCode::Enter]);
        crate::testing::settle(&mut app);
        let bag = app.world().get::<Inventory>(me).unwrap().items.clone();
        let names: Vec<(String, u32)> =
            bag.iter().map(|i| (app.world().get::<Name>(*i).unwrap().as_str().to_owned(), app.world().get::<Stack>(*i).map_or(1, |s| s.count))).collect();
        assert_eq!(names, vec![("frag grenade".to_owned(), 2)]);
        assert_eq!(top(&app), Some(SEARCH), "and the search stays up for the next");
        let said: Vec<String> = app.world().resource::<MessageLog>().iter().filter(|e| e.text.contains("frag grenade")).map(|e| e.text.clone()).collect();
        assert_eq!(said, ["You pick up a frag grenade.", "You pick up 2 frag grenades."], "and the narrator says what arrived, once for each");
        press(&mut app, KeyCode::Escape);
        assert_eq!(top(&app), Some(MENU), "escape steps back to the menu");
    }

    /// What is typed into the search is typed, not played: `l` is a step
    /// east to the world and `i` the pack, and neither happens.
    #[test]
    fn letters_typed_into_the_search_never_reach_the_world() {
        let (mut app, me) = playing();
        let at = app.world().get::<Position>(me).unwrap().0;
        let clock = crate::testing::clock(&app);
        keys(&mut app, &[KeyCode::Backslash, KeyCode::KeyS, KeyCode::KeyL, KeyCode::KeyI, KeyCode::KeyH]);
        assert_eq!(top(&app), Some(SEARCH), "still searching");
        assert_eq!(app.world().get::<Position>(me).unwrap().0, at, "no step taken");
        assert_eq!(crate::testing::clock(&app), clock, "and no turn spent");
        assert_eq!(app.world().resource::<CheatScreen>().query, "lih");
    }
}
