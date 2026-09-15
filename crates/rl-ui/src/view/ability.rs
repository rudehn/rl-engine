//! What an actor can do with its turn, what each thing does, and what is
//! stopping it.
//!
//! The list every game with abilities writes: the names, in a stable
//! order, with the ones that cannot be used greyed and a reason beside
//! them. The reasons come from the same gate the resolver runs, through
//! [`Offered`], so a row is never greyed for something the resolver would
//! have allowed and never offered for something it would refuse.
//!
//! Everything else on a row comes from the ability's own definition, which
//! is data: the description the game wrote, the shape and the aim, what a
//! use costs, what it needs, how long it takes and how long until it can
//! be used again, and what each effect does in its own words through
//! [`Effect::describe`]. A game that adds an ability in RON gets its menu
//! entry for free; one that adds an effect writes one method to have it
//! described.
//!
//! Where a row needs a word the engine does not have, the word is a
//! registry's: a cost is phrased as `12 mana` because the game registered
//! a stat called mana. A game that wants other phrasing pushes a [`Facet`]
//! in [`ViewSet::Annotate`](crate::ViewSet).

use bevy::prelude::*;
use rl_bevy::Offered;
use rl_bevy::prelude::*;
use rl_grid::TargetMode;
use rl_rules::ability::{AbilityId, Aim, Blocked, Cost, Requirement};

use crate::facet::Facet;

/// One ability, as a menu reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct AbilityRow {
    /// Which.
    pub ability: AbilityId,
    /// Its registered name.
    pub label: String,
    /// What it is, in the game's words. Empty when the game wrote none.
    pub description: String,
    /// What it wants under it, so a menu may show a self ability apart.
    pub aim: Aim,
    /// Its shape, range included.
    pub mode: TargetMode,
    /// What the turn costs, in hundredths of a step.
    pub time: u32,
    /// Hundredths before it may be used again, zero for none.
    pub cooldown: u32,
    /// What a use spends, each phrased with the registry's name: `12 mana`,
    /// `1 powder`, `3 health`.
    pub costs: Vec<String>,
    /// What must be true of the user, phrased the same way.
    pub requires: Vec<String>,
    /// What each effect does, in the words its effect gave.
    pub effects: Vec<String>,
    /// Why it cannot be used, empty when it can.
    pub blocked: Vec<Blocked>,
    /// The same, in words: `needs 12 mana`, `ready in 2 turns`.
    pub why: Vec<String>,
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
    registries: Option<Res<Registries>>,
    turns: Res<Turns>,
    holders: Query<(Entity, &Known, Option<&Cooldowns>), With<MyTurn>>,
) {
    view.rows.clear();
    view.entity = None;
    let (Some(abilities), Some(registries)) = (abilities.as_deref(), registries.as_deref()) else { return };
    let Ok((actor, known, cooldowns)) = holders.single() else { return };
    view.entity = Some(actor);
    let now = turns.now();
    for (id, _) in known.iter() {
        let def = abilities.get(id);
        let ready_at = cooldowns.map(|c| c.ready_at(id)).unwrap_or(0);
        let cooling = ready_at.saturating_sub(now);
        // The gate's answer when it has one for this actor. A frame in
        // which nobody holds a turn leaves the reasons empty rather than
        // guessing at them.
        let blocked: Vec<Blocked> = offered.as_deref().map(|o| o.why_for(actor, id).to_vec()).unwrap_or_default();
        let why = blocked.iter().map(|b| phrase_blocked(b, cooling, registries)).collect();
        view.rows.push(AbilityRow {
            ability: id,
            label: def.name.clone(),
            description: def.description.clone(),
            aim: def.aim,
            mode: def.mode,
            time: def.time,
            cooldown: def.cooldown,
            costs: def.costs.iter().map(|c| phrase_cost(c, registries)).collect(),
            requires: def.requires.iter().map(|r| phrase_requirement(r, registries)).collect(),
            effects: abilities.describe(id, registries),
            blocked,
            why,
            cooling,
            facets: Vec::new(),
        });
    }
}

/// A cost, with the registry's name for what it is a cost in.
pub fn phrase_cost(cost: &Cost, registries: &Registries) -> String {
    match cost {
        Cost::Pool { stat, amount } => format!("{amount} {}", registries.stats.name(*stat)),
        Cost::Charge { amount } => format!("{amount} {}", if *amount == 1 { "charge" } else { "charges" }),
        Cost::Health { amount } => format!("{amount} health"),
        Cost::Item { tag, count } => format!("{count} {}", registries.tags.name(*tag)),
    }
}

/// A requirement, with the registry's names.
pub fn phrase_requirement(requirement: &Requirement, registries: &Registries) -> String {
    match requirement {
        Requirement::Has(status) => format!("{} on you", registries.statuses.name(*status)),
        Requirement::Lacks(status) => format!("not {}", registries.statuses.name(*status)),
        Requirement::Wielding(tag) => format!("{} equipped", registries.tags.name(*tag)),
        Requirement::InSlot(slot, tag) => format!("{} in the {}", registries.tags.name(*tag), registries.slots.name(*slot)),
        Requirement::Above(stat, n) => format!("{} above {n}", registries.stats.name(*stat)),
    }
}

/// A reason, in words a player can act on. `cooling` is the hundredths
/// left, for the one reason that is a wait.
pub fn phrase_blocked(reason: &Blocked, cooling: u32, registries: &Registries) -> String {
    match reason {
        Blocked::Cooling { .. } => format!("ready in {}", turns(cooling)),
        Blocked::Cannot(cost) => format!("needs {}", phrase_cost(cost, registries)),
        Blocked::Needs(requirement) => format!("needs {}", phrase_requirement(requirement, registries)),
        Blocked::NoTarget => "no target".to_string(),
    }
}

/// Hundredths of a step as turns, a whole number when it is one.
pub fn turns(hundredths: u32) -> String {
    let whole = hundredths.div_ceil(100);
    if whole == 1 { "1 turn".to_string() } else { format!("{whole} turns") }
}

/// A plain phrase for a reason, for a banner with no room for more.
///
/// Deliberately vague: the words that name the cost are on the row's
/// `why`, and a banner one line high has no room for them.
pub fn plain(reason: &Blocked) -> &'static str {
    match reason {
        Blocked::Cooling { .. } => "not ready",
        Blocked::Cannot(_) => "cannot pay",
        Blocked::Needs(_) => "missing something",
        Blocked::NoTarget => "no target",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;
    use crate::view::target::harness::{abilities, arm};
    use rl_bevy::AddEngineEffects;

    fn staged() -> Stage {
        let mut stage = Stage::new_with((AbilitiesPlugin, AbilityViewPlugin), |app| {
            app.add_engine_effects();
            abilities(app);
        });
        stage.tick();
        arm(&mut stage);
        stage.tick();
        stage
    }

    /// Every row says what it does, what it costs and how far it reaches,
    /// out of the data the ability was written in.
    #[test]
    fn a_row_describes_the_ability_from_its_definition() {
        let stage = staged();
        let view = stage.app.world().resource::<AbilityView>();
        let bolt = view.rows.iter().find(|r| r.label == "bolt").expect("known");
        assert_eq!(bolt.description, "a bolt of force", "the game's own words");
        assert_eq!(bolt.costs, vec!["5 focus"], "the registry's name for the pool");
        assert_eq!(bolt.effects, vec!["3 kinetic"], "what the effect says of itself");
        assert_eq!(bolt.mode, TargetMode::Bolt { range: 6 });
        assert!(bolt.why.is_empty() && bolt.ready());
        let dear = view.rows.iter().find(|r| r.label == "dear").expect("known");
        assert_eq!(dear.why, vec!["needs 99 focus"], "and why not, in the same words");
        let burst = view.rows.iter().find(|r| r.label == "burst").expect("known");
        assert_eq!(burst.description, "", "no description written, none invented");
        assert_eq!(burst.effects, vec!["2 kinetic"]);
    }

    #[test]
    fn hundredths_read_as_whole_turns_rounded_up() {
        assert_eq!(turns(100), "1 turn");
        assert_eq!(turns(250), "3 turns");
        assert_eq!(turns(0), "0 turns");
    }
}
