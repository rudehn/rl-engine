//! Cues: what a turn did that is worth seeing, told to whoever draws, and
//! the hold on the turns while it is shown.
//!
//! The turn loop is instant: a bolt is cast, flies, lands and kills in
//! one frame, and a player sees a crab die of nothing. A resolver that did
//! something visible writes a [`Cued`]: a [`Cue::Flight`] from one
//! [`Anchor`] to another, or a [`Cue::Burst`] on some. An anchor is a cell
//! or an entity standing on one, and one that follows an entity goes where
//! the entity goes: a pulse on someone who steps aside steps aside with
//! them, and a bolt at a player who walks on lands on the player, not on
//! the floor they left.
//!
//! Nothing is drawn here. A plugin that plays cues, `ParticlesPlugin` in
//! `rl-render`, says so with [`TurnHold::watch`], and from then on the
//! loop stops after any pass that cued something and runs no pass until
//! the plugin lets go. That is what makes a fight legible: the crab's spit
//! is seen to fly before the eel moves, and two monsters casting are two
//! flights, one after the other. A cue at the player holds like any
//! other, so the spit is seen to arrive before it hurts; what keeps the
//! player from waiting on it is the plugin, which lets a key pressed
//! while the turns wait skip through to the player's turn. Without a
//! watcher the cues are written and forgotten, and the loop runs as if
//! there were none, which is what a headless game gets.
//!
//! A flight lands when it has been seen. A resolver that cued one, with
//! something watching, puts what the flight does aside and tells the hold
//! it has [`launched`](TurnHold::launch); no turn is dealt while anything
//! is in the air, and the first pass after the hold lets go lands it,
//! which is when the fire starts and the blow falls, and cues the burst
//! that is then waited on in turn. Without a watcher nothing is put
//! aside and a use lands the moment it is cast, as it always did.

use bevy::prelude::*;
use rl_core::Point;
use rl_rules::ability::{AbilityId, Look};

/// Where a cue plays: a cell, or an entity's cell as it stands each frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Anchor {
    /// The cell, and where the cue plays if the entity it follows is gone.
    pub at: Point,
    /// The entity it goes with, if one.
    pub follow: Option<Entity>,
}

impl Anchor {
    /// A cell.
    pub fn cell(at: Point) -> Self {
        Self { at, follow: None }
    }

    /// `who`, standing on `at` when the cue was written.
    pub fn on(who: Entity, at: Point) -> Self {
        Self { at, follow: Some(who) }
    }
}

/// What a cue is drawn as: something the drawer looks up, or a look given
/// outright.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LookOf {
    /// The ability's own look, or the drawer's plain one if it has none.
    Ability(AbilityId),
    /// The item's glyph.
    Item(Entity),
    /// This.
    Given(Look),
    /// Whatever the drawer uses when nothing says.
    Plain,
}

/// One thing worth seeing.
#[derive(Debug, Clone, PartialEq)]
pub enum Cue {
    /// Something flies from one anchor to another.
    Flight {
        /// Where it leaves from.
        from: Anchor,
        /// Where it lands.
        to: Anchor,
        /// What flies.
        look: LookOf,
    },
    /// Something happens on every anchor at once.
    Burst {
        /// Where.
        on: Vec<Anchor>,
        /// What shows.
        look: LookOf,
    },
}

/// A cue, and whose act it was. Cues from one actor in one pass play one
/// after another, in the order written: a flight, then the burst where it
/// landed, then whatever an effect added, such as what a drain gives back.
#[derive(Message, Debug, Clone, PartialEq)]
pub struct Cued {
    /// Who did it.
    pub actor: Entity,
    /// What is worth seeing.
    pub cue: Cue,
}

/// Whether the turn loop is waiting for something to be seen.
///
/// Held only while something watches: a plugin that plays cues calls
/// [`watch`](Self::watch) once, and the loop is then stopped by
/// [`hold_for_cues`] after a pass that cued something and freed by the
/// plugin when the last of it has played. A game with no such plugin never
/// waits.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct TurnHold {
    watched: bool,
    held: bool,
    /// Flights cued and not yet landed.
    in_flight: u32,
}

impl TurnHold {
    /// Lets go of everything for a new run, keeping whether anything
    /// watches: what was in the air belonged to the run that ended.
    pub fn reset(&mut self) {
        self.held = false;
        self.in_flight = 0;
    }

    /// Something will play the cues and let go when they are done.
    pub fn watch(&mut self) {
        self.watched = true;
    }

    /// Whether anything watches.
    pub fn is_watched(&self) -> bool {
        self.watched
    }

    /// Stops the loop, if anything watches to start it again.
    pub fn hold(&mut self) {
        self.held = self.watched;
    }

    /// Lets the loop run on.
    pub fn release(&mut self) {
        self.held = false;
    }

    /// Whether the loop is stopped.
    pub fn is_held(&self) -> bool {
        self.held
    }

    /// Something has been cued to fly whose landing waits on being seen.
    pub fn launch(&mut self) {
        self.in_flight += 1;
    }

    /// One of them has landed.
    pub fn land(&mut self) {
        self.in_flight = self.in_flight.saturating_sub(1);
    }

    /// Whether anything is in the air, during which no turn is dealt.
    pub fn in_flight(&self) -> bool {
        self.in_flight > 0
    }
}

/// Stops the turns after a pass that cued something, so the next actor
/// waits until it has been seen. Runs in the pass's cleanup, and does
/// nothing unless something watches.
pub fn hold_for_cues(mut cues: MessageReader<Cued>, mut hold: ResMut<TurnHold>) {
    if cues.read().next().is_some() && hold.is_watched() {
        hold.hold();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{Actor, Blocks, MyTurn, Player, Position};
    use crate::turn::{Intent, Resolution, Wait};

    #[test]
    fn a_hold_only_takes_while_something_watches() {
        let mut hold = TurnHold::default();
        hold.hold();
        assert!(!hold.is_held(), "nobody would let go");
        hold.watch();
        hold.hold();
        assert!(hold.is_held());
        hold.release();
        assert!(!hold.is_held());
    }

    type Holding<'w, 's> = Query<'w, 's, (Entity, &'static Position), (With<MyTurn>, Without<Player>)>;
    type Players<'w, 's> = Query<'w, 's, (Entity, &'static Position), With<Player>>;

    /// Every non-player cues its own turn: a burst on itself, or on the
    /// player when it stands beside them.
    fn cue_own_turn(mut resolution: Resolution, mut cues: MessageWriter<Cued>, holding: Holding, players: Players) {
        for (actor, at) in &holding {
            if !resolution.claim(actor) {
                continue;
            }
            let beside = players.iter().find(|(_, p)| rl_core::geometry::chebyshev(p.0, at.0) == 1).map(|(p, at)| Anchor::on(p, at.0));
            cues.write(Cued { actor, cue: Cue::Burst { on: vec![beside.unwrap_or(Anchor::on(actor, at.0))], look: LookOf::Plain } });
            resolution.done(actor, 100);
        }
    }

    fn cued_by(app: &mut App) -> Vec<Entity> {
        app.world_mut().resource_mut::<Messages<Cued>>().drain().map(|c| c.actor).collect()
    }

    /// With something watching, one cue stops the loop until it is let
    /// go, and the next actor's cue then stops it again: two monsters
    /// casting are seen one after the other, and one at the player is
    /// seen to arrive like any other.
    #[test]
    fn the_turns_wait_on_each_cue_in_turn_the_ones_at_the_player_included() {
        let mut app = crate::plugin::headless_app();
        app.add_plugins((crate::fov::FovPlugin, crate::world::StreamingPlugin));
        let start = crate::testing::surface(&mut app);
        app.add_systems(crate::plugin::Turn, cue_own_turn.in_set(crate::plugin::ResolveSet::Act));
        let me = app.world_mut().spawn((Actor, Player, Blocks, Position(start), crate::components::Viewshed::new(8), crate::components::RevealsMap)).id();
        let near = app.world_mut().spawn((Actor, Blocks, Position(start.offset(1, 0)))).id();
        let far = app.world_mut().spawn((Actor, Blocks, Position(start.offset(3, 0)))).id();
        let farther = app.world_mut().spawn((Actor, Blocks, Position(start.offset(5, 0)))).id();
        app.world_mut().resource_mut::<NextState<crate::state::EngineState>>().set(crate::state::EngineState::Playing);
        app.world_mut().resource_mut::<TurnHold>().watch();
        app.update();
        app.update();
        assert!(app.world().get::<MyTurn>(me).is_some(), "the player goes first");

        app.world_mut().write_message(Intent::new(me, Wait));
        app.update();
        assert_eq!(cued_by(&mut app), vec![near], "the one beside the player cued the player, and that held the loop");
        assert!(app.world().resource::<TurnHold>().is_held());
        app.update();
        assert!(cued_by(&mut app).is_empty(), "nothing acts while it is held");
        app.world_mut().resource_mut::<TurnHold>().release();
        app.update();
        assert_eq!(cued_by(&mut app), vec![far], "let go, the next one acts, and is waited on in turn");
        assert!(app.world().resource::<TurnHold>().is_held());
        app.world_mut().resource_mut::<TurnHold>().release();
        app.update();
        assert_eq!(cued_by(&mut app), vec![farther]);
        app.world_mut().resource_mut::<TurnHold>().release();
        app.update();
        assert!(app.world().get::<MyTurn>(me).is_some(), "and the turn comes back round");
    }
}
