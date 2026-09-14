//! What an actor can do with its turn, and what is stopping it.
//!
//! The list every game with abilities writes: the names, in a stable
//! order, with the ones that cannot be used greyed and a reason beside
//! them. The reasons come from the same gate the resolver runs, through
//! [`Offered`], so a row is never greyed for something the resolver would
//! have allowed and never offered for something it would refuse.
//!
//! Only the words are the game's. The engine has no name for a cost in
//! mana or a missing shield, so a row carries the [`Blocked`] values and a
//! [`Facets`](crate::facet::Facets) hook for the game to phrase them; the
//! panel prints a plain fallback when the game says nothing.

use bevy::prelude::*;
use rl_bevy::prelude::*;
use rl_rules::ability::{AbilityId, Aim, Blocked};

use crate::facet::Facet;

/// One ability, as a menu reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct AbilityRow {
    /// Which.
    pub ability: AbilityId,
    /// Its registered name.
    pub label: String,
    /// What it wants under it, so a menu may show a self ability apart.
    pub aim: Aim,
    /// Why it cannot be used, empty when it can.
    pub blocked: Vec<Blocked>,
    /// Hundredths of a step until it is ready, zero when it is.
    pub cooling: u32,
    /// What the game added.
    pub facets: Vec<Facet>,
}

impl AbilityRow {
    /// Whether it can be used right now.
    pub fn ready(&self) -> bool {
        self.blocked.is_empty()
    }
}

/// Everything the turn-holder knows, in a stable order.
#[derive(Resource, Debug, Default)]
pub struct AbilityView {
    /// Whose abilities these are.
    pub entity: Option<Entity>,
    /// The rows, in registration order, so a key bound to the third row
    /// stays bound to the third row.
    pub rows: Vec<AbilityRow>,
}

impl AbilityView {
    /// The row at `index`, for a menu cursor.
    pub fn row(&self, index: usize) -> Option<&AbilityRow> {
        self.rows.get(index)
    }

    /// How many can be used right now.
    pub fn ready(&self) -> usize {
        self.rows.iter().filter(|r| r.ready()).count()
    }
}

/// Keeps [`AbilityView`] current.
///
/// Nothing here runs in a game that never added
/// [`AbilitiesPlugin`]: without [`Abilities`]
/// there is nothing to list, and the view stays empty rather than the
/// plugin insisting on a registry the game has no use for.
pub struct AbilityViewPlugin;

impl Plugin for AbilityViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AbilityView>().add_systems(Update, collect_abilities.in_set(crate::ViewSet::Collect));
    }

    fn finish(&self, app: &mut App) {
        rl_bevy::depends_on::<crate::UiPlugin>(app, "AbilityViewPlugin");
    }
}

/// Fills [`AbilityView`] from what the turn-holder knows.
pub fn collect_abilities(
    mut view: ResMut<AbilityView>,
    abilities: Option<Res<Abilities>>,
    offered: Option<Res<Offered>>,
    turns: Res<Turns>,
    holders: Query<(Entity, &Known, Option<&Cooldowns>), With<MyTurn>>,
) {
    view.rows.clear();
    view.entity = None;
    let Some(abilities) = abilities.as_deref() else { return };
    let Ok((actor, known, cooldowns)) = holders.single() else { return };
    view.entity = Some(actor);
    let now = turns.now();
    for (id, _) in known.iter() {
        let def = abilities.get(id);
        let ready_at = cooldowns.map(|c| c.ready_at(id)).unwrap_or(0);
        view.rows.push(AbilityRow {
            ability: id,
            label: def.name.clone(),
            aim: def.aim,
            // The gate's answer when it has one for this actor. A frame
            // in which nobody holds a turn leaves the reasons empty
            // rather than guessing at them.
            blocked: offered.as_deref().map(|o| o.why_for(actor, id).to_vec()).unwrap_or_default(),
            cooling: ready_at.saturating_sub(now),
            facets: Vec::new(),
        });
    }
}

/// A plain phrase for a reason, for a panel with nothing better.
///
/// Deliberately vague: the engine knows an unaffordable cost was a pool
/// and not which, because the stat is the game's word. A game that wants
/// "out of mana" pushes a facet in [`ViewSet::Annotate`](crate::ViewSet::Annotate).
pub fn plain(reason: &Blocked) -> &'static str {
    match reason {
        Blocked::Cooling { .. } => "not ready",
        Blocked::Cannot(_) => "cannot pay",
        Blocked::Needs(_) => "missing something",
        Blocked::NoTarget => "no target",
    }
}
