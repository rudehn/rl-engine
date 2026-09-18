//! The character sheet: every number the player is made of, and where
//! each came from.
//!
//! The vitals strip says what the player has; the sheet says why. Each
//! registered stat with its base, its final value and every modifier
//! between them, tagged by source; what the player resists; the blows it
//! lands; the statuses on it with the turns they have left and what they
//! do; and what it wears, slot by slot. All of it is engine state already,
//! which is what makes it a view rather than a game's screen.
//!
//! What the engine can name of a modifier's source, it does: a status's
//! carries the status, and a worn item's carries the item, which the sheet
//! names by its [`Name`]. A source of the game's own is the game's to name,
//! in [`ViewSet::Annotate`](crate::ViewSet) through
//! [`SheetView::name_source`], and one nobody named is shown by its effect
//! alone.

use bevy::prelude::*;
use rl_bevy::prelude::*;
use rl_core::DiceRoll;
use rl_render::Glyph;
use rl_rules::damage::DamageKindId;
use rl_rules::stats::{Op, Source};
use rl_rules::{SlotId, StatId, StatusId};

use crate::facet::Facet;

/// One modifier on a stat, as the sheet reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// What it does.
    pub op: Op,
    /// What applied it: a status, a worn item, or something of the game's.
    pub source: Source,
    /// What applied it, in words, when known: the status's name, the worn
    /// item's, or what the game named the source. Empty when nobody did.
    pub from: String,
}

/// One registered stat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatLine {
    /// Which.
    pub stat: StatId,
    /// Its registered name.
    pub name: String,
    /// The value with nothing on it.
    pub base: i32,
    /// The value with everything on it, the number the rules use.
    pub value: i32,
    /// Everything between the two, in the order it was applied.
    pub changes: Vec<Change>,
}

/// Resistance to one damage kind, as a percentage removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResistLine {
    /// Which kind.
    pub kind: DamageKindId,
    /// Its registered name.
    pub name: String,
    /// Percent removed: 100 immune, negative vulnerable.
    pub pct: i32,
}

/// One roll the player lands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Strike {
    /// The damage kind's registered name.
    pub kind: String,
    /// The roll.
    pub dice: DiceRoll,
    /// How far it reaches, for a shot; `None` for a blow.
    pub range: Option<i32>,
}

/// One status on the player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusLine {
    /// Which.
    pub status: StatusId,
    /// Its registered name.
    pub name: String,
    /// Whole turns left.
    pub turns: u32,
    /// What it does to which stat, by the stat's name.
    pub modifies: Vec<(String, Op)>,
    /// Damage it deals a turn, by the kind's name.
    pub ticks: Option<(String, i32)>,
}

/// One equipment slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WornLine {
    /// Which.
    pub slot: SlotId,
    /// Its registered name.
    pub name: String,
    /// What is in it, by name, if anything.
    pub item: Option<String>,
}

/// Everything the player is made of.
#[derive(Resource, Debug, Default)]
pub struct SheetView {
    /// The player, while there is one.
    pub entity: Option<Entity>,
    /// What the game called it.
    pub label: String,
    /// How it is drawn, if it is.
    pub glyph: Option<Glyph>,
    /// Current and maximum health.
    pub health: Option<(i32, i32)>,
    /// Flat armor.
    pub armor: Option<i32>,
    /// Speed as a percentage of normal.
    pub speed: Option<u32>,
    /// Every registered stat, in registration order.
    pub stats: Vec<StatLine>,
    /// Every registered damage kind the player resists or is vulnerable
    /// to, in registration order. Kinds at zero are left out.
    pub resists: Vec<ResistLine>,
    /// The blows and the shot, main blow first.
    pub strikes: Vec<Strike>,
    /// The statuses on it, in the order they were applied.
    pub statuses: Vec<StatusLine>,
    /// Every registered slot, in registration order.
    pub worn: Vec<WornLine>,
    /// What the game added.
    pub facets: Vec<Facet>,
}

impl SheetView {
    /// Names every change tagged `source` as coming `from` something:
    /// what a game's annotate system calls for each source of its own.
    pub fn name_source(&mut self, source: Source, from: impl Into<String>) {
        let from = from.into();
        for change in self.stats.iter_mut().flat_map(|s| s.changes.iter_mut()).filter(|c| c.source == source) {
            change.from = from.clone();
        }
    }

    /// The stat called `name`, if the game registered one.
    pub fn stat(&self, name: &str) -> Option<&StatLine> {
        self.stats.iter().find(|s| s.name == name)
    }
}

/// Keeps [`SheetView`] current.
///
/// Needs [`Registries`], for the names every line carries. Every part of
/// the player is optional: a player with no [`StatBlock`] lists no stats
/// and one with no [`Equipped`] no slots.
pub struct SheetViewPlugin;

impl Plugin for SheetViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SheetView>()
            .needs::<Registries>("SheetViewPlugin", "`Registries`, for the names of the stats, kinds, statuses and slots the sheet lists")
            .add_systems(Update, collect_sheet.in_set(crate::ViewSet::Collect));
    }

    fn finish(&self, app: &mut App) {
        rl_bevy::depends_on::<crate::UiPlugin>(app, "SheetViewPlugin");
    }
}

/// What the collector reads off the player.
type Made = (
    Entity,
    Option<&'static Name>,
    Option<&'static Glyph>,
    Option<&'static Health>,
    Option<&'static Armor>,
    Option<&'static Speed>,
    Option<&'static StatBlock>,
    Option<&'static Resists>,
);
/// What the player carries and suffers.
type Armed = (Option<&'static Afflicted>, Option<&'static Equipped>);

/// Fills [`SheetView`] from the player.
///
/// Armor and the strikes come from the player's [`Loadout`], so the sheet
/// shows what a worn blade adds without the game copying it anywhere.
pub fn collect_sheet(
    mut view: ResMut<SheetView>,
    registries: Res<Registries>,
    loadout: Loadout,
    player: Query<(Made, Armed), With<Player>>,
    names: Query<&Name>,
) {
    *view = SheetView::default();
    let Ok(((entity, name, glyph, health, armor, speed, stats, resists), (afflicted, worn))) = player.single() else { return };
    view.entity = Some(entity);
    view.label = name.map(|n| n.as_str().to_string()).unwrap_or_default();
    view.glyph = glyph.copied();
    view.health = health.map(|h| (h.current, h.max));
    let total_armor = loadout.armor(entity);
    view.armor = (armor.is_some() || total_armor != 0).then_some(total_armor);
    view.speed = speed.map(|s| s.0);

    if let Some(stats) = stats {
        for (stat, def) in registries.stats.iter() {
            let changes = stats
                .modifiers()
                .iter()
                .filter(|m| m.stat == stat)
                .map(|m| Change { op: m.op, source: m.source, from: source_named(m.source, afflicted, &registries, &names) })
                .collect();
            view.stats.push(StatLine {
                stat,
                name: def.name.clone(),
                base: stats.base(stat, &registries.stats),
                value: stats.value(stat, &registries.stats),
                changes,
            });
        }
    }
    if let Some(resists) = resists {
        for (kind, def) in registries.damage_kinds.iter() {
            let pct = resists.get(kind);
            if pct != 0 {
                view.resists.push(ResistLine { kind, name: def.name.clone(), pct });
            }
        }
    }
    let kind_name = |kind: DamageKindId| registries.damage_kinds.name(kind).to_string();
    for (kind, dice) in loadout.blows(entity) {
        view.strikes.push(Strike { kind: kind_name(kind), dice, range: None });
    }
    if let Some(ranged) = loadout.ranged(entity) {
        view.strikes.push(Strike { kind: kind_name(ranged.kind), dice: ranged.dice, range: Some(ranged.range) });
    }
    if let Some(afflicted) = afflicted {
        for active in afflicted.iter() {
            let def = registries.statuses.get(active.id);
            view.statuses.push(StatusLine {
                status: active.id,
                name: def.name.clone(),
                turns: active.turns,
                modifies: def.modifiers.iter().map(|m| (registries.stats.name(m.stat).to_string(), m.op)).collect(),
                ticks: def.tick_damage.map(|(kind, amount)| (kind_name(kind), amount)),
            });
        }
    }
    if let Some(worn) = worn {
        for (slot, def) in registries.slots.iter() {
            let item = worn.in_slot(slot).and_then(|e| names.get(e).ok()).map(|n| n.as_str().to_string());
            view.worn.push(WornLine { slot, name: def.name.clone(), item });
        }
    }
}

/// What the engine can say applied a modifier: the status's registered
/// name while it is still on, the worn item's [`Name`], and nothing for a
/// source of the game's own, which the game names.
fn source_named(source: Source, afflicted: Option<&Afflicted>, registries: &Registries, names: &Query<&Name>) -> String {
    match source {
        Source::Status { status, .. } if afflicted.is_some_and(|a| a.has(status)) => registries.statuses.name(status).to_string(),
        Source::Item(bits) => Entity::try_from_bits(bits).and_then(|item| names.get(item).ok()).map(|n| n.as_str().to_string()).unwrap_or_default(),
        Source::Status { .. } | Source::Game(_) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;
    use rl_rules::content::Registry;
    use rl_rules::{StatDef, StatusDef, stats::Modifier};

    /// A stage whose player has stats, a resistance, a shot and a slot.
    fn stage() -> (Stage, StatId) {
        let mut stage = Stage::new_with(SheetViewPlugin, |app| {
            let mut registries = app.world_mut().resource_mut::<Registries>();
            registries.stats = Registry::from_defs(vec![StatDef::new("might", 10), StatDef::new("wit", 3)]).unwrap();
            let might = registries.stats.expect("might");
            registries.statuses =
                Registry::from_defs(vec![StatusDef::new("weak").modifies(might, Op::Add(-3)).ticks(rl_rules::damage::DamageKindId::from_raw(0), 1)]).unwrap();
            registries.slots = Registry::from_defs(vec![rl_rules::SlotDef::new("hand"), rl_rules::SlotDef::new("head")]).unwrap();
        });
        let (player, kind) = (stage.player, stage.kind);
        let might = stage.app.world().resource::<Registries>().stats.expect("might");
        let mut stats = rl_rules::Stats::new();
        stats.add(Modifier::new(might, Op::Add(2), Source::Game(77)));
        let mut resists = rl_rules::Resistances::new();
        resists.set(kind, 25);
        stage.app.world_mut().entity_mut(player).insert((
            StatBlock(stats),
            Resists(resists),
            Afflicted::default(),
            Speed(120),
            RangedAttack::new(kind, DiceRoll::flat(3), 6),
            Strikes(vec![(kind, DiceRoll::new(1, 4))]),
        ));
        stage.tick();
        (stage, might)
    }

    #[test]
    fn the_sheet_reads_every_number_off_the_player_with_its_name() {
        let (stage, _) = stage();
        let view = stage.app.world().resource::<SheetView>();
        assert_eq!(view.label, "you");
        assert_eq!(view.glyph.map(|g| g.ch), Some('@'));
        assert_eq!((view.health, view.armor, view.speed), (Some((30, 30)), Some(0), Some(120)));
        let might = view.stat("might").expect("registered");
        assert_eq!((might.base, might.value), (10, 12), "the base, and the value with the modifier on it");
        assert_eq!(might.changes.len(), 1);
        assert_eq!(might.changes[0].from, "", "the game's tag, which nobody has named yet");
        assert_eq!(view.stat("wit").map(|s| (s.base, s.value, s.changes.len())), Some((3, 3, 0)));
        assert_eq!(view.resists.iter().map(|r| (r.name.as_str(), r.pct)).collect::<Vec<_>>(), vec![("kinetic", 25)]);
        assert_eq!(view.strikes.len(), 3, "the blow, the extra strike and the shot");
        assert_eq!(view.strikes[0].range, None);
        assert_eq!(view.strikes[2].range, Some(6));
        assert_eq!(view.worn.iter().map(|w| w.name.as_str()).collect::<Vec<_>>(), Vec::<&str>::new(), "no Equipped, no slots");
    }

    #[test]
    fn a_status_names_its_own_changes_and_the_game_names_the_rest() {
        let (mut stage, might) = stage();
        let player = stage.player;
        let weak = stage.app.world().resource::<Registries>().statuses.expect("weak");
        stage.app.world_mut().write_message(Afflict { target: player, status: weak, turns: 4, by: None });
        stage.app.add_systems(Update, (|mut view: ResMut<SheetView>| view.name_source(Source::Game(77), "a ring")).in_set(crate::ViewSet::Annotate));
        stage.tick();
        stage.tick();
        let view = stage.app.world().resource::<SheetView>();
        let line = view.stat("might").expect("registered");
        assert_eq!(line.value, 9, "10 + 2 - 3");
        let from: Vec<&str> = line.changes.iter().map(|c| c.from.as_str()).collect();
        assert_eq!(from, vec!["a ring", "weak"], "{:?}", line.changes);
        assert_eq!(line.stat, might);
        assert_eq!(view.statuses.len(), 1);
        let status = &view.statuses[0];
        assert_eq!((status.name.as_str(), status.turns), ("weak", 4));
        assert_eq!(status.modifies, vec![("might".to_string(), Op::Add(-3))]);
        assert_eq!(status.ticks, Some(("kinetic".to_string(), 1)));
    }

    /// A worn blade is the blow the sheet lists, its armor is in the total,
    /// and what it bestows is a change the sheet names after the blade,
    /// with nothing copied onto the player and no annotate system naming
    /// anything.
    #[test]
    fn worn_slots_are_listed_in_registration_order_and_worn_gear_counts_by_name() {
        let (mut stage, might) = stage();
        let (player, kind) = (stage.player, stage.kind);
        let hand = stage.app.world().resource::<Registries>().slots.expect("hand");
        let blade = stage
            .app
            .world_mut()
            .spawn((Item, Name::new("a blade"), Armor(2), MeleeAttack::new(kind, DiceRoll::new(2, 6)), Bestows(vec![(might, Op::Add(5))])))
            .id();
        let mut worn = Equipped(rl_rules::Equipment::with_slot_count(2));
        worn.equip(blade, &rl_rules::EquipShape::in_slot(hand)).expect("the slot exists");
        stage.app.world_mut().entity_mut(player).insert(worn);
        // One pass of the turn loop folds the gear; the frame after draws it.
        stage.app.world_mut().write_message(Intent::new(player, Wait));
        stage.tick();
        stage.tick();
        let view = stage.app.world().resource::<SheetView>();
        let lines: Vec<(&str, Option<&str>)> = view.worn.iter().map(|w| (w.name.as_str(), w.item.as_deref())).collect();
        assert_eq!(lines, vec![("hand", Some("a blade")), ("head", None)]);
        assert_eq!(view.armor, Some(2), "the blade's armor, on a player with none of its own");
        assert_eq!(view.strikes[0].dice, DiceRoll::new(2, 6), "the blade is the blow, not the fist");
        let line = view.stat("might").expect("registered");
        assert_eq!(line.value, 10 + 2 + 5);
        assert_eq!(line.changes.last().map(|c| c.from.as_str()), Some("a blade"), "{:?}", line.changes);
    }
}
