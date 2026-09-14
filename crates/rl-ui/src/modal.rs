//! One thing at a time: the stack of open screens, and the run conditions
//! that read it.
//!
//! Every game with an inventory discovers the same bug: the player walks
//! while the bag is open, or two screens both think they own the arrow
//! keys. The fix is that exactly one screen holds input, that opening a
//! second remembers the first, and that closing returns to it. That is a
//! stack, and it belongs to the engine because every game gets it wrong on
//! its own.
//!
//! A stack rather than a "return to" slot: a slot can be pushed twice and
//! lose the first target, which is a bug a stack cannot have.
//!
//! ```
//! # use bevy::prelude::*;
//! # use rl_ui::{Modals, UiPlugin, modal_is, no_modal};
//! # fn bag_keys() {}
//! # fn walk() {}
//! let mut app = App::new();
//! app.add_plugins(UiPlugin);
//! // Declare while building, keep the id, gate systems on it.
//! let bag = app.world_mut().resource_mut::<Modals>().declare("bag");
//! app.add_systems(Update, bag_keys.run_if(modal_is(bag)));
//! app.add_systems(Update, walk.run_if(no_modal));
//! ```

use bevy::prelude::*;
use rl_core::{Id, Interner};

/// The type [`ModalId`] indexes. Never constructed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Modal;

/// A registered modal.
pub type ModalId = Id<Modal>;

/// Which screens are open, innermost last.
///
/// Empty means the world has input. The engine never opens or closes
/// anything here; a game's key handler does, and the run conditions below
/// read it.
#[derive(Resource, Debug, Clone, Default)]
pub struct Modals {
    names: Interner<Modal>,
    stack: Vec<ModalId>,
}

impl Modals {
    /// The id for `name`, assigning a new one if it is unseen.
    ///
    /// Declare while the app is being built: a run condition needs the id
    /// before there is a world to look it up in.
    pub fn declare(&mut self, name: &str) -> ModalId {
        self.names.intern(name)
    }

    /// The id for `name`, if it has been declared.
    pub fn get(&self, name: &str) -> Option<ModalId> {
        self.names.get(name)
    }

    /// The name behind `modal`.
    ///
    /// # Panics
    /// Panics if `modal` did not come from this registry.
    pub fn name(&self, modal: ModalId) -> &str {
        self.names.name(modal)
    }

    /// Opens `modal` over whatever is open now.
    ///
    /// Opening one that is already on the stack raises it to the top rather
    /// than listing it twice, so a key pressed during a frame that already
    /// opened the screen cannot stack it on itself.
    pub fn open(&mut self, modal: ModalId) {
        self.stack.retain(|m| *m != modal);
        self.stack.push(modal);
    }

    /// Closes the top screen and returns to the one under it, if any.
    pub fn close(&mut self) -> Option<ModalId> {
        self.stack.pop()
    }

    /// Closes everything and gives input back to the world. What an action
    /// that ends a turn does, since the turn loop assumes no screen is up.
    pub fn close_all(&mut self) {
        self.stack.clear();
    }

    /// Closes `modal` wherever it is in the stack, leaving the rest.
    pub fn close_one(&mut self, modal: ModalId) {
        self.stack.retain(|m| *m != modal);
    }

    /// Opens `modal` if it is closed, closes it if it is open. What a
    /// toggle key does.
    pub fn toggle(&mut self, modal: ModalId) {
        if self.is_open(modal) {
            self.close_one(modal);
        } else {
            self.open(modal);
        }
    }

    /// The screen on top, the one that owns input and draws last.
    pub fn top(&self) -> Option<ModalId> {
        self.stack.last().copied()
    }

    /// Whether `modal` is anywhere on the stack.
    pub fn is_open(&self, modal: ModalId) -> bool {
        self.stack.contains(&modal)
    }

    /// Whether `modal` is the one on top.
    pub fn is_top(&self, modal: ModalId) -> bool {
        self.top() == Some(modal)
    }

    /// Whether anything is open.
    pub fn any_open(&self) -> bool {
        !self.stack.is_empty()
    }

    /// The stack, outermost first. Panels draw in this order.
    pub fn stack(&self) -> &[ModalId] {
        &self.stack
    }
}

/// A run condition: true while `modal` is the screen on top.
///
/// On top rather than merely open, so a screen that opened a child stops
/// reading keys until the child closes.
pub fn modal_is(modal: ModalId) -> impl Fn(Res<Modals>) -> bool + Clone {
    move |modals: Res<Modals>| modals.is_top(modal)
}

/// A run condition: true while `modal` is open, under a child or not.
/// What a panel that should keep drawing behind a child screen uses.
pub fn modal_open(modal: ModalId) -> impl Fn(Res<Modals>) -> bool + Clone {
    move |modals: Res<Modals>| modals.is_open(modal)
}

/// A run condition: true while nothing is open. Gate the game's own
/// movement keys on this and the player cannot walk with the bag up.
pub fn no_modal(modals: Res<Modals>) -> bool {
    !modals.any_open()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plugin that declares a modal may be listed before the plugin that
    /// owns the stack: the order a game adds its plugins in is not something
    /// it should have to know.
    #[test]
    fn a_plugin_that_declares_a_modal_may_come_before_the_ui_plugin() {
        let mut app = rl_bevy::plugin::headless_app();
        app.add_plugins((crate::view::inspect::InspectViewPlugin, crate::UiPlugin));
        app.finish();
        assert!(app.world().resource::<Modals>().get(crate::view::inspect::INSPECT_MODAL).is_some(), "the modal survived the stack's own plugin");
    }

    #[test]
    fn a_child_takes_input_and_closing_it_returns_to_the_parent() {
        let mut modals = Modals::default();
        let bag = modals.declare("bag");
        let detail = modals.declare("detail");
        modals.open(bag);
        assert!(modals.is_top(bag));
        modals.open(detail);
        assert!(modals.is_top(detail));
        assert!(modals.is_open(bag), "the parent is still open, just not on top");
        assert!(!modals.is_top(bag), "and does not read keys while the child is up");
        modals.close();
        assert!(modals.is_top(bag), "closing the child returns to the parent");
        modals.close();
        assert!(!modals.any_open(), "and closing that gives input back to the world");
    }

    #[test]
    fn opening_the_same_screen_twice_raises_it_instead_of_stacking_it() {
        let mut modals = Modals::default();
        let bag = modals.declare("bag");
        let map = modals.declare("map");
        modals.open(bag);
        modals.open(map);
        modals.open(bag);
        assert_eq!(modals.stack(), &[map, bag]);
        modals.close();
        assert!(modals.is_top(map), "one close, not two");
    }

    #[test]
    fn an_action_that_ends_a_turn_clears_the_whole_stack() {
        let mut modals = Modals::default();
        let bag = modals.declare("bag");
        let detail = modals.declare("detail");
        modals.open(bag);
        modals.open(detail);
        modals.close_all();
        assert!(!modals.any_open());
        assert_eq!(modals.top(), None);
    }
}
