//! What the player carries, item by item, with what each one is worth.
//!
//! The list every game with items writes: the bag in pickup order, which of
//! it is worn and where, how many of a stack there are, and the numbers on
//! each thing. The numbers are the item's own components, the ones
//! [`Loadout`] reads when a blow is struck: a blade's [`MeleeAttack`], a
//! coat's [`Armor`], what a ring [`Bestows`], how far a knife flies. So an
//! item spawned with them is described for free, in the registries' names,
//! and a game says nothing twice.
//!
//! What an item does is its [`Triggers`], one line per effect, each named
//! by the moment that sets it off: `use: mends 5`, `on landing: 3 kinetic
//! in a burst of 1`, and how its use is pointed when it is used at
//! somewhere. What a use costs it is its [`Consumable`], counted on the
//! row when it holds more than one charge, and a use that asks for another
//! of the player's things lists what it may be made on. Anything else an
//! item means is a [`Facet`] the game pushes in
//! [`ViewSet::Annotate`](crate::ViewSet), and the panel prints it under the
//! row.
//!
//! A thing [`Unknown`] reaches its row by its name and where it goes and
//! nothing else, not even whether it is empty, so a game's identification
//! is a name and a marker rather than a redaction of every row in
//! `Annotate`; a worn thing that is [`Stuck`] says so, for the keys that
//! would take it off. A game that knows part of an unknown thing, its kind
//! and not its level, says what it seems in [`ItemRow::seems`], which a
//! panel prints in place of the bare `unknown`, and may write the numbers
//! it seems to have into the row in `Annotate`, which a panel prints as it
//! prints a known thing's.

use bevy::prelude::*;
use rl_bevy::MomentId;
use rl_bevy::prelude::*;
use rl_render::Glyph;
use rl_rules::SlotId;
use rl_rules::stats::Op;

use crate::facet::Facet;
use crate::view::sheet::{ResistLine, Strike};

/// One carried item, as a bag screen reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemRow {
    /// Which.
    pub entity: Entity,
    /// What the game called it, from its [`Name`]; empty for an item with
    /// none, which is listed rather than hidden because it is carried
    /// whether or not it was named.
    pub label: String,
    /// How it is drawn on the map, if it is.
    pub glyph: Option<Glyph>,
    /// How many, for a stack; one otherwise.
    pub count: u32,
    /// The slot it is worn in, if it is worn.
    pub slot: Option<SlotId>,
    /// That slot's registered name, empty when not worn.
    pub slot_name: String,
    /// Where it would go if put on: the registered names of the slots it
    /// may take, empty for something that cannot be worn.
    pub goes_on: Vec<String>,
    /// The slots it takes as well wherever it goes, by their registered
    /// names: a two-hander's off hand. Empty for most things.
    pub also_takes: Vec<String>,
    /// How far it flies if thrown, when it can be.
    pub throw_range: Option<i32>,
    /// What it strikes for when thrown, when a throw is a blow.
    pub thrown: Option<Strike>,
    /// Flat armor while worn.
    pub armor: i32,
    /// What it resists while worn, a percentage by damage kind, negative
    /// for what it leaves its wearer weaker to; nothing for a thing that
    /// resists nothing.
    pub resists: Vec<ResistLine>,
    /// The blow it is swung with, if wielding it strikes.
    pub blow: Option<Strike>,
    /// The shot it fires, if wielding it shoots.
    pub shot: Option<Strike>,
    /// The extra rolls every hit carries while it is worn.
    pub strikes: Vec<Strike>,
    /// What it does, one line per effect that has something to say, each
    /// named by the moment that sets it off: `use: mends 5`, `on landing:
    /// 3 kinetic in a burst of 1`. Empty for most of a bag.
    pub used: Vec<String>,
    /// Whether it has a `use` trigger at all, which is not the same as
    /// having something to say about it.
    pub uses_something: bool,
    /// How its use is pointed, for a thing used at somewhere rather than on
    /// whoever uses it: the bag asks for the targeting cursor for it rather
    /// than using it at once, and says how far it reaches.
    pub targeting: Option<rl_rules::Targeting>,
    /// What its use asks for, for a thing used on another of its user's
    /// things: the bag chooses for it, under its prompt, rather than using
    /// it at once.
    pub asks: Option<Asking>,
    /// Charges left and the most it holds, for a thing that holds more than
    /// one; a stack of single-charge things is counted by `count` instead.
    pub charges: Option<(u16, u16)>,
    /// Whether it is a consumable with nothing left in the unit in hand,
    /// as the player knows it: never for a thing not yet known, whose
    /// charges are as hidden as what it does.
    pub empty: bool,
    /// Whole turns until the next charge comes back, for a thing that
    /// refills, is not full, and whose clock is running.
    pub ready_in: Option<u32>,
    /// Whether its charges come back only while it is worn.
    pub attuned: bool,
    /// What it does to registered stats while worn, by the stat's name.
    pub bestows: Vec<(String, Op)>,
    /// What it counts as, by the tags' registered names.
    pub tags: Vec<String>,
    /// What the game added.
    pub facets: Vec<Facet>,
    /// Whether it is a thing not yet known, [`Unknown`]: the row carries
    /// its name and where it goes and nothing it does or is worth, and a
    /// panel says it is unknown in their place. What the keys act on stays,
    /// since its real triggers still work, and its use key stays while it
    /// is empty, since a key gone would say it was.
    pub unknown: bool,
    /// What the game says it seems, while it is not yet known: lines a
    /// panel prints in place of the bare `unknown`, in their own tones, such
    /// as that its level is not yet known and how long until it will be.
    /// Pushed in [`ViewSet::Annotate`](crate::ViewSet) as facets are, and
    /// read only while `unknown`, since a thing known needs no word on what
    /// it seems. Empty, the panel says `unknown`.
    pub seems: Vec<Facet>,
    /// Whether it is worn and will not come off, [`Stuck`]: the bag offers
    /// no key that would take it off, and says so. In the bag a stuck thing
    /// is not, since only being worn holds it.
    pub stuck: bool,
}

/// What a use that asks for one of its user's things asks for, as a bag
/// reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asking {
    /// What the choice is asked under, in the game's words.
    pub prompt: String,
    /// What it may be made on, in the bag's order, by
    /// [`may_use_on`](rl_bevy::may_use_on), the rule the items resolver
    /// takes a choice by, so the bag offers nothing it would refuse.
    pub choices: Vec<Entity>,
}

impl ItemRow {
    /// Whether it is on.
    pub fn worn(&self) -> bool {
        self.slot.is_some()
    }

    /// Whether it can be put on.
    pub fn wearable(&self) -> bool {
        !self.goes_on.is_empty()
    }

    /// Whether the use key does anything to it: a `use` trigger, a charge
    /// to spend when it counts them, for a thing that can be worn, being
    /// worn, which is when the items resolver lets it be used, and for one
    /// whose use asks for a thing, something to choose.
    ///
    /// A thing whose effects all keep quiet about themselves is still used:
    /// the row reads it off the component, not off the description, so a
    /// game that wrote a terse effect does not lose the key that uses it.
    pub fn usable(&self) -> bool {
        let something_to_choose = self.asks.as_ref().is_none_or(|a| !a.choices.is_empty());
        self.uses_something && !self.empty && (!self.wearable() || self.worn()) && something_to_choose
    }

    /// The row with what an unknown thing does and is worth taken out, and
    /// what the bag's keys act on left in. Whether it is empty is out with
    /// its charges, so its use key stays while it is empty: a key that went
    /// away would say so, and the use is made and comes to nothing.
    fn withheld(self) -> Self {
        Self {
            empty: false,
            thrown: None,
            armor: 0,
            resists: Vec::new(),
            blow: None,
            shot: None,
            strikes: Vec::new(),
            used: Vec::new(),
            charges: None,
            ready_in: None,
            attuned: false,
            bestows: Vec::new(),
            tags: Vec::new(),
            unknown: true,
            ..self
        }
    }
}

/// The player's bag.
#[derive(Resource, Debug, Default)]
pub struct InventoryView {
    /// Whose bag, while there is a player with one.
    pub entity: Option<Entity>,
    /// The items, in pickup order, worn or not.
    pub rows: Vec<ItemRow>,
}

impl InventoryView {
    /// The row at `index`, for a menu cursor.
    pub fn row(&self, index: usize) -> Option<&ItemRow> {
        self.rows.get(index)
    }

    /// The row for `item`, for a game annotating one it recognises.
    pub fn row_of(&mut self, item: Entity) -> Option<&mut ItemRow> {
        self.rows.iter_mut().find(|r| r.entity == item)
    }
}

/// Keeps [`InventoryView`] current.
///
/// Needs nothing. A player with no [`Inventory`] lists nothing, and the
/// names on each row come from [`Registries`] when the game inserted one
/// and are empty otherwise.
pub struct InventoryViewPlugin;

impl Plugin for InventoryViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InventoryView>().add_systems(Update, collect_inventory.in_set(crate::ViewSet::Collect));
    }

    fn finish(&self, app: &mut App) {
        rl_bevy::depends_on::<crate::UiPlugin>(app, "InventoryViewPlugin");
    }
}

/// How an item looks and where it goes.
type Looks =
    (Option<&'static Name>, Option<&'static Glyph>, Option<&'static Stack>, Option<&'static Wearable>, Option<&'static Throwable>, Option<&'static Tagged>);
/// What an item does when worn: the same components [`Loadout`] reads.
type Arms = (
    Option<&'static Armor>,
    Option<&'static Resists>,
    Option<&'static MeleeAttack>,
    Option<&'static RangedAttack>,
    Option<&'static Strikes>,
    Option<&'static Bestows>,
);
/// What an item does at its moments, what a use costs it, its clock,
/// whether its charges come back only while worn, and its `EffectBonus`.
type Does =
    (Option<&'static Triggers>, Option<&'static Consumable>, Option<&'static rl_bevy::Pulse>, Has<rl_bevy::Attuned>, Option<&'static rl_bevy::EffectBonus>);
/// Whether an item is not yet known, and whether it will not come off.
type Marks = (Has<rl_bevy::Unknown>, Has<rl_bevy::Stuck>);

/// How a trigger's lines are introduced on a bag's row: by what the player
/// does to set it off, in the engine's own moments, and by the moment's
/// name for one a game registered. A pulse says how often it comes round.
fn lead_in(moment: MomentId, moments: Option<&Moments>, pulse: Option<&rl_bevy::Pulse>) -> String {
    match moment {
        m if m == Moments::USE => "use".to_string(),
        // Not "thrown": the row already says how far it flies, and the
        // word twice over reads as two things.
        m if m == Moments::LAND => "on landing".to_string(),
        m if m == Moments::HIT => "on a hit".to_string(),
        m if m == Moments::FIRE => "when fired".to_string(),
        m if m == Moments::PULSE => match pulse {
            // "Every 1 turn" is a count nobody says aloud.
            Some(p) if p.every <= rl_core::turn::BASE_ACTION_COST => "every turn worn".to_string(),
            Some(p) => format!("every {} worn", crate::view::ability::turns(p.every)),
            None => "worn".to_string(),
        },
        m => moments.map(|all| all.name(m).to_string()).unwrap_or_default(),
    }
}

/// What a carrier's triggers do, in the effects' own words, one line per
/// effect that has something to say, led by the moment that sets it off.
///
/// A `use` trigger and a `land` trigger that would say the same lines, a
/// thing used or thrown to the same end, say them once under `use or
/// thrown` rather than twice under their own names; any other trigger, or
/// a `use` and a `land` whose lines differ, keeps its own lead-in. The
/// merge only ever looks at a carrier with exactly one of each: a second
/// `use` trigger (legal unless the first is aimed or asks for a thing) or
/// a second `land` trigger (always legal) means its triggers are each
/// their own, not a drink-or-throw pair, so every one of them keeps its
/// own name rather than a second one being printed under the first's
/// lines or dropped for repeating it.
fn describe_uses(triggers: &Triggers, registries: &Registries, moments: Option<&Moments>, pulse: Option<&rl_bevy::Pulse>, bonus: EffectBonus) -> Vec<String> {
    let lines_of = |trigger: &rl_bevy::Trigger| -> Vec<String> {
        let area = match trigger.area {
            rl_rules::Area::Here => String::new(),
            rl_rules::Area::Burst { radius } => format!(" in a burst of {radius}"),
        };
        trigger.effects.describe(registries, bonus).into_iter().map(|line| format!("{line}{area}")).collect()
    };
    // The one trigger on `moment`, and `None` for none or for more than
    // one: a second of either takes every one of its moment out of the
    // merge below, since there is no longer a single pair to fold.
    let only_on = |moment: MomentId| {
        let mut on = triggers.on(moment);
        on.next().filter(|_| on.next().is_none())
    };
    // The shared lines, when the carrier's one `use` trigger and one
    // `land` trigger both carry something and say the same thing; `None`
    // leaves every trigger under its own name, including a `use` or a
    // `land` alone.
    let shared = match (only_on(Moments::USE), only_on(Moments::LAND)) {
        (Some(use_trigger), Some(land_trigger)) => {
            let (use_lines, land_lines) = (lines_of(use_trigger), lines_of(land_trigger));
            (!use_lines.is_empty() && use_lines == land_lines).then_some(use_lines)
        }
        _ => None,
    };
    let mut used = Vec::new();
    for trigger in &triggers.0 {
        if shared.is_some() && trigger.on == Moments::LAND {
            continue; // said once already, under the `use` trigger below.
        }
        match (&shared, trigger.on == Moments::USE) {
            (Some(lines), true) => used.extend(lines.iter().map(|line| format!("use or thrown: {line}"))),
            _ => {
                let lead = lead_in(trigger.on, moments, pulse);
                used.extend(lines_of(trigger).into_iter().map(|line| format!("{lead}: {line}")));
            }
        }
    }
    used
}

/// Fills [`InventoryView`] from the player's bag.
pub fn collect_inventory(
    mut view: ResMut<InventoryView>,
    registries: Option<Res<Registries>>,
    moments: Option<Res<Moments>>,
    player: Query<(Entity, &Inventory, Option<&Equipped>), With<Player>>,
    items: Query<(Looks, Arms, Does, Marks), With<Item>>,
    tags: Query<&Tagged>,
) {
    view.rows.clear();
    view.entity = None;
    let Ok((entity, bag, worn)) = player.single() else { return };
    view.entity = Some(entity);
    let registries = registries.as_deref();
    let moments = moments.as_deref();
    let kind_name = |kind| registries.map(|r| r.damage_kinds.name(kind).to_string()).unwrap_or_default();
    let slot_name = |slot| registries.map(|r| r.slots.name(slot).to_string()).unwrap_or_default();
    let strike = |(kind, dice): (rl_rules::damage::DamageKindId, rl_core::DiceRoll), range: Option<i32>| Strike { kind: kind_name(kind), dice, range };
    for &item in &bag.items {
        let Ok((
            (name, glyph, stack, wearable, throwable, tagged),
            (armor, resists, melee, ranged, strikes, bestows),
            (triggers, consumable, pulse, attuned, bonus),
            (unknown, stuck),
        )) = items.get(item)
        else {
            continue;
        };
        let slot = worn.and_then(|w| w.slot_of(item));
        // What its use may be made on, by the rule the resolver takes a
        // choice by, among the things the bag lists.
        let asks = triggers.and_then(Triggers::asks).map(|asks| Asking {
            prompt: asks.prompt.clone(),
            choices: bag.items.iter().copied().filter(|on| items.contains(*on) && rl_bevy::may_use_on(asks, item, *on, bag, tags.get(*on).ok())).collect(),
        });
        // What it does, in the effects' own words: the same lines the
        // ability list shows under an ability, since they are the same
        // effects described by the same code.
        let used = match (triggers, registries) {
            (Some(triggers), Some(registries)) => describe_uses(triggers, registries, moments, pulse, bonus.copied().unwrap_or_default()),
            _ => Vec::new(),
        };
        let clock_runs = !attuned || slot.is_some();
        let ready_in = consumable
            .filter(|c| c.left < c.max && clock_runs)
            .and_then(|c| c.recharge)
            .map(|r| (r.every - r.progress.min(r.every)).div_ceil(rl_core::turn::BASE_ACTION_COST));
        let row = ItemRow {
            entity: item,
            used,
            uses_something: triggers.is_some_and(|t| t.on(Moments::USE).next().is_some()),
            targeting: triggers.and_then(Triggers::targeting),
            asks,
            charges: consumable.filter(|c| c.max > 1).map(|c| (c.left, c.max)),
            empty: consumable.is_some_and(|c| c.is_empty()),
            label: name.map(|n| n.as_str().to_string()).unwrap_or_default(),
            glyph: glyph.copied(),
            count: stack.map_or(1, |s| s.count),
            slot,
            slot_name: slot.map(slot_name).unwrap_or_default(),
            goes_on: wearable.map(|w| w.0.any_of.iter().map(|s| slot_name(*s)).collect()).unwrap_or_default(),
            also_takes: wearable.map(|w| w.0.also.iter().map(|s| slot_name(*s)).collect()).unwrap_or_default(),
            throw_range: throwable.map(|t| t.range),
            thrown: throwable.and_then(|t| t.strike.map(|s| strike(s, Some(t.range)))),
            armor: armor.map_or(0, |a| a.0),
            resists: match (resists, registries) {
                (Some(r), Some(registries)) => registries
                    .damage_kinds
                    .iter()
                    .filter(|(kind, _)| r.0.get(*kind) != 0)
                    .map(|(kind, def)| ResistLine { kind, name: def.name.clone(), pct: r.0.get(kind) })
                    .collect(),
                _ => Vec::new(),
            },
            blow: melee.map(|m| strike((m.kind, m.dice), None)),
            shot: ranged.map(|r| strike((r.kind, r.dice), Some(r.range))),
            strikes: strikes.map(|s| s.0.iter().map(|hit| strike(*hit, None)).collect()).unwrap_or_default(),
            bestows: bestows
                .map(|b| b.0.iter().map(|(stat, op)| (registries.map(|r| r.stats.name(*stat).to_string()).unwrap_or_default(), *op)).collect())
                .unwrap_or_default(),
            tags: tagged.map(|t| t.0.iter().map(|tag| registries.map(|r| r.tags.name(*tag).to_string()).unwrap_or_default()).collect()).unwrap_or_default(),
            facets: Vec::new(),
            seems: Vec::new(),
            ready_in,
            attuned,
            unknown: false,
            stuck: stuck && slot.is_some(),
        };
        view.rows.push(if unknown { row.withheld() } else { row });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;
    use rl_core::DiceRoll;
    use rl_rules::content::Registry;
    use rl_rules::{EquipShape, Equipment, SlotDef, StatDef};

    /// A row carries the item's own numbers in the registries' names, and
    /// says where it is worn and where it could be.
    #[test]
    fn the_bag_is_listed_in_pickup_order_with_what_each_thing_is_worth() {
        let mut stage = Stage::new_with(InventoryViewPlugin, |app| {
            let mut registries = app.world_mut().resource_mut::<Registries>();
            registries.stats = Registry::from_defs(vec![StatDef::new("might", 10)]).unwrap();
            registries.slots = Registry::from_defs(vec![SlotDef::new("hand"), SlotDef::new("head")]).unwrap();
        });
        let (player, kind) = (stage.player, stage.kind);
        let (hand, head, might) = {
            let r = stage.app.world().resource::<Registries>();
            (r.slots.expect("hand"), r.slots.expect("head"), r.stats.expect("might"))
        };
        let blade = stage
            .app
            .world_mut()
            .spawn((
                Item,
                Name::new("a blade"),
                Wearable(EquipShape::in_slot(hand)),
                MeleeAttack::new(kind, DiceRoll::new(1, 6)),
                Strikes(vec![(kind, DiceRoll::flat(1))]),
                Bestows(vec![(might, Op::Add(2))]),
            ))
            .id();
        let mut resists = rl_rules::Resistances::new();
        resists.set(kind, 10);
        let hat = stage.app.world_mut().spawn((Item, Name::new("a hat"), Wearable(EquipShape::in_slot(head)), Armor(1), Resists(resists))).id();
        let knives =
            stage.app.world_mut().spawn((Item, Name::new("knife"), Stack { key: 1, count: 3 }, Throwable::new(5, Some((kind, DiceRoll::new(1, 4)))))).id();
        let mut worn = Equipped(Equipment::with_slot_count(2));
        worn.equip(blade, &EquipShape::in_slot(hand)).unwrap();
        stage.app.world_mut().entity_mut(player).insert((Inventory { items: vec![knives, blade, hat] }, worn));
        stage.tick();

        let view = stage.app.world().resource::<InventoryView>();
        assert_eq!(view.entity, Some(player));
        let labels: Vec<&str> = view.rows.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(labels, vec!["knife", "a blade", "a hat"], "pickup order, worn or not, each by its name for one");

        let knives = &view.rows[0];
        assert_eq!((knives.count, knives.throw_range), (3, Some(5)));
        assert_eq!(knives.thrown.as_ref().map(|s| (s.kind.as_str(), s.range)), Some(("kinetic", Some(5))));
        assert!(!knives.wearable() && !knives.worn());

        let blade = &view.rows[1];
        assert_eq!((blade.slot, blade.slot_name.as_str()), (Some(hand), "hand"), "worn, and where");
        assert_eq!(blade.blow.as_ref().map(|s| s.dice), Some(DiceRoll::new(1, 6)));
        assert_eq!(blade.strikes.len(), 1);
        assert_eq!(blade.bestows, vec![("might".to_string(), Op::Add(2))], "by the stat's name");

        let hat = &view.rows[2];
        assert!(!hat.worn() && hat.wearable());
        assert_eq!((hat.goes_on.as_slice(), hat.armor), (["head".to_string()].as_slice(), 1));
        assert_eq!(hat.resists.iter().map(|r| (r.name.as_str(), r.pct)).collect::<Vec<_>>(), vec![("kinetic", 10)], "by the kind's name");
        assert!(blade.resists.is_empty(), "and nothing for a thing that resists nothing");
    }

    /// The bag offers the use key for a worn thing only while it is on,
    /// the rule the items resolver refuses it by.
    #[test]
    fn a_wearable_thing_is_usable_only_while_it_is_worn() {
        let mut stage = Stage::new_with(InventoryViewPlugin, |app| {
            app.world_mut().resource_mut::<Registries>().slots = Registry::from_defs(vec![SlotDef::new("torso")]).unwrap();
        });
        let player = stage.player;
        let torso = stage.app.world().resource::<Registries>().slots.expect("torso");
        let on_use = rl_bevy::Trigger {
            on: rl_bevy::Moments::USE,
            area: rl_rules::Area::Here,
            fires: None,
            effects: std::sync::Arc::new(rl_bevy::Effects::default()),
            look: None,
            every: None,
            targeting: None,
            asks: None,
        };
        let plate = stage.app.world_mut().spawn((Item, Name::new("plate"), Wearable(EquipShape::in_slot(torso)), rl_bevy::Triggers(vec![on_use]))).id();
        stage.app.world_mut().entity_mut(player).insert((Inventory { items: vec![plate] }, Equipped(Equipment::with_slot_count(1))));
        stage.tick();
        assert!(!stage.app.world().resource::<InventoryView>().rows[0].usable(), "in the bag, the use key is not offered");
        stage.app.world_mut().get_mut::<Equipped>(player).unwrap().equip(plate, &EquipShape::in_slot(torso)).unwrap();
        stage.tick();
        assert!(stage.app.world().resource::<InventoryView>().rows[0].usable(), "worn, it is");
    }

    /// A pulse's lead-in says how often it comes round, in whole turns
    /// rounded up, so it never promises a mend before its clock has come
    /// round.
    #[test]
    fn a_pulse_is_introduced_by_how_often_it_comes_round() {
        assert_eq!(lead_in(Moments::PULSE, None, Some(&rl_bevy::Pulse::every(800))), "every 8 turns worn");
        assert_eq!(lead_in(Moments::PULSE, None, Some(&rl_bevy::Pulse::every(750))), "every 8 turns worn", "a part turn rounds up, never promising early");
        assert_eq!(lead_in(Moments::PULSE, None, Some(&rl_bevy::Pulse::every(100))), "every turn worn", "one turn is no count at all");
        assert_eq!(lead_in(Moments::PULSE, None, Some(&rl_bevy::Pulse::every(50))), "every turn worn", "nor is less than one");
    }

    /// A row's effects are described with its item's own `EffectBonus`
    /// added: a plain `Inflict` of ten turns and a carrier bonus of four
    /// reads fourteen.
    #[test]
    fn a_rows_effects_are_described_with_its_effectbonus_added() {
        let mut stage = Stage::new_with(InventoryViewPlugin, |app| {
            app.world_mut().resource_mut::<Registries>().statuses = Registry::from_defs(vec![rl_rules::StatusDef::new("cloaked")]).unwrap();
        });
        let player = stage.player;
        let registries = stage.app.world().resource::<Registries>().clone();
        let mut kinds = EffectKinds::default();
        kinds.declare::<rl_bevy::Inflict>();
        let args = rl_rules::ability::parse_args(r#"(status: "cloaked", turns: 10)"#).unwrap();
        let specs = vec![rl_rules::EffectSpec { kind: "Inflict".to_string(), chance: 100, args }];
        let effects = Effects::build(&specs, &kinds, &registries.names()).unwrap();
        let trigger = rl_bevy::Trigger {
            on: Moments::USE,
            area: rl_rules::Area::Here,
            fires: None,
            effects: std::sync::Arc::new(effects),
            look: None,
            every: None,
            targeting: None,
            asks: None,
        };
        let plate = stage.app.world_mut().spawn((Item, Name::new("cloak plate"), Triggers(vec![trigger]), EffectBonus { turns: 4, amount: 0 })).id();
        stage.app.world_mut().entity_mut(player).insert(Inventory { items: vec![plate] });
        stage.tick();
        let view = stage.app.world().resource::<InventoryView>();
        assert!(view.rows[0].used.iter().any(|line| line == "use: cloaked for 14 turns"), "{:?}", view.rows[0].used);
    }

    /// A trigger on `on` that lands `effects` with nothing else about it.
    fn plain_trigger(on: MomentId, effects: &std::sync::Arc<Effects>) -> rl_bevy::Trigger {
        rl_bevy::Trigger { on, area: rl_rules::Area::Here, fires: None, effects: effects.clone(), look: None, every: None, targeting: None, asks: None }
    }

    /// A `Mend` effect list of one roll, built the way an item's would be.
    fn mend(registries: &Registries, kinds: &EffectKinds, roll: &str) -> std::sync::Arc<Effects> {
        let args = rl_rules::ability::parse_args(&format!(r#"(roll: "{roll}")"#)).unwrap();
        let specs = vec![rl_rules::EffectSpec { kind: "Mend".to_string(), chance: 100, args }];
        std::sync::Arc::new(Effects::build(&specs, kinds, &registries.names()).unwrap())
    }

    /// A thing used or thrown to the same end says so once, under `use
    /// or thrown`, where it used to list the same mend under `use` and
    /// again under `on landing`; a thing whose throw heals for a
    /// different amount than its drink keeps the two lines apart.
    #[test]
    fn a_use_and_a_land_trigger_that_say_the_same_thing_are_said_once() {
        let mut stage = Stage::new(InventoryViewPlugin);
        let player = stage.player;
        let registries = stage.app.world().resource::<Registries>().clone();
        let mut kinds = EffectKinds::default();
        kinds.declare::<rl_bevy::Mend>();
        let (shared, different) = (mend(&registries, &kinds, "3d6+4"), mend(&registries, &kinds, "1d4"));
        let ember = stage
            .app
            .world_mut()
            .spawn((Item, Name::new("ember"), Triggers(vec![plain_trigger(Moments::USE, &shared), plain_trigger(Moments::LAND, &shared)])))
            .id();
        let odd = stage
            .app
            .world_mut()
            .spawn((Item, Name::new("pebble"), Triggers(vec![plain_trigger(Moments::USE, &different), plain_trigger(Moments::LAND, &shared)])))
            .id();
        stage.app.world_mut().entity_mut(player).insert(Inventory { items: vec![ember, odd] });
        stage.tick();
        let view = stage.app.world().resource::<InventoryView>();
        assert_eq!(view.rows[0].used, vec!["use or thrown: mends 3d6+4".to_string()], "said once, not twice over: {:?}", view.rows[0].used);
        assert_eq!(
            view.rows[1].used,
            vec!["use: mends 1d4".to_string(), "on landing: mends 3d6+4".to_string()],
            "a use and a land that differ keep their own lead-in: {:?}",
            view.rows[1].used
        );
    }

    /// Two `use` triggers, a weapon's plain second shot or the like
    /// rather than an aimed or asking one, are each their own even when
    /// the first says the same thing a `land` trigger would: a carrier
    /// may hold more than one `use`, and merging would print the second
    /// under the first's lines rather than its own.
    #[test]
    fn two_use_triggers_are_never_merged_even_when_one_matches_the_lone_land_trigger() {
        let mut stage = Stage::new(InventoryViewPlugin);
        let player = stage.player;
        let registries = stage.app.world().resource::<Registries>().clone();
        let mut kinds = EffectKinds::default();
        kinds.declare::<rl_bevy::Mend>();
        let (shared, different) = (mend(&registries, &kinds, "3d6+4"), mend(&registries, &kinds, "1d4"));
        let ember = stage
            .app
            .world_mut()
            .spawn((
                Item,
                Name::new("ember"),
                Triggers(vec![plain_trigger(Moments::USE, &shared), plain_trigger(Moments::USE, &different), plain_trigger(Moments::LAND, &shared)]),
            ))
            .id();
        stage.app.world_mut().entity_mut(player).insert(Inventory { items: vec![ember] });
        stage.tick();
        let view = stage.app.world().resource::<InventoryView>();
        assert_eq!(
            view.rows[0].used,
            vec!["use: mends 3d6+4".to_string(), "use: mends 1d4".to_string(), "on landing: mends 3d6+4".to_string()],
            "a second use trigger of its own, not printed under the first's lines nor folded into the land: {:?}",
            view.rows[0].used
        );
    }

    /// Two `land` triggers are each their own even when the first says
    /// the same thing the lone `use` trigger would: merging would drop
    /// the second one for repeating the first rather than saying it too.
    #[test]
    fn two_land_triggers_are_never_merged_even_when_one_matches_the_lone_use_trigger() {
        let mut stage = Stage::new(InventoryViewPlugin);
        let player = stage.player;
        let registries = stage.app.world().resource::<Registries>().clone();
        let mut kinds = EffectKinds::default();
        kinds.declare::<rl_bevy::Mend>();
        let (shared, different) = (mend(&registries, &kinds, "3d6+4"), mend(&registries, &kinds, "1d4"));
        let ember = stage
            .app
            .world_mut()
            .spawn((
                Item,
                Name::new("ember"),
                Triggers(vec![plain_trigger(Moments::USE, &shared), plain_trigger(Moments::LAND, &shared), plain_trigger(Moments::LAND, &different)]),
            ))
            .id();
        stage.app.world_mut().entity_mut(player).insert(Inventory { items: vec![ember] });
        stage.tick();
        let view = stage.app.world().resource::<InventoryView>();
        assert_eq!(
            view.rows[0].used,
            vec!["use: mends 3d6+4".to_string(), "on landing: mends 3d6+4".to_string(), "on landing: mends 1d4".to_string()],
            "a second land trigger of its own, not dropped for repeating the first nor merged with the use: {:?}",
            view.rows[0].used
        );
    }

    /// A `use` trigger with nothing in it, aimed or asking as given.
    fn on_use(asks: Option<rl_rules::Asks>) -> rl_bevy::Trigger {
        let effects = std::sync::Arc::new(rl_bevy::Effects::default());
        rl_bevy::Trigger { on: Moments::USE, area: rl_rules::Area::Here, fires: None, effects, look: None, every: None, targeting: None, asks }
    }

    /// A thing not yet known reaches the row by its name and where it goes,
    /// and nothing it does or is worth does: no effect, no blow, no armor,
    /// nothing it bestows or counts as, no charges. What the bag's keys act
    /// on stays, since its real triggers still work; the same thing known
    /// is listed whole.
    #[test]
    fn an_unknown_things_row_carries_its_name_and_slot_and_nothing_it_does_or_is_worth() {
        let mut stage = Stage::new_with(InventoryViewPlugin, |app| {
            let mut registries = app.world_mut().resource_mut::<Registries>();
            registries.stats = Registry::from_defs(vec![StatDef::new("might", 10)]).unwrap();
            registries.slots = Registry::from_defs(vec![SlotDef::new("hand")]).unwrap();
            registries.statuses = Registry::from_defs(vec![rl_rules::StatusDef::new("cloaked")]).unwrap();
            registries.tags = Registry::from_defs(vec![rl_rules::TagDef::new("kit")]).unwrap();
        });
        let (player, kind) = (stage.player, stage.kind);
        let registries = stage.app.world().resource::<Registries>().clone();
        let (hand, might, kit) = (registries.slots.expect("hand"), registries.stats.expect("might"), registries.tags.expect("kit"));
        let mut kinds = EffectKinds::default();
        kinds.declare::<rl_bevy::Inflict>();
        let args = rl_rules::ability::parse_args(r#"(status: "cloaked", turns: 10)"#).unwrap();
        let specs = vec![rl_rules::EffectSpec { kind: "Inflict".to_string(), chance: 100, args }];
        let cloak = rl_bevy::Trigger { effects: std::sync::Arc::new(Effects::build(&specs, &kinds, &registries.names()).unwrap()), ..on_use(None) };
        let rod = |name: &str| {
            (
                (Item, Name::new(name.to_string()), Wearable(EquipShape::in_slot(hand)), Tagged(vec![kit])),
                (MeleeAttack::new(kind, DiceRoll::new(1, 6)), Armor(2), Bestows(vec![(might, Op::Add(2))])),
                (Triggers(vec![cloak.clone()]), Consumable { left: 3, ..Consumable::new(5, WhenEmpty::Kept) }),
            )
        };
        let known = stage.app.world_mut().spawn(rod("ash rod")).id();
        let unknown = stage.app.world_mut().spawn((rod("teak rod"), rl_bevy::Unknown)).id();
        stage.app.world_mut().entity_mut(player).insert(Inventory { items: vec![known, unknown] });
        stage.tick();

        let view = stage.app.world().resource::<InventoryView>();
        let (known, unknown) = (&view.rows[0], &view.rows[1]);
        assert!(!known.unknown && known.blow.is_some() && known.armor == 2 && !known.bestows.is_empty(), "known, it is listed whole");
        assert!(!known.used.is_empty() && known.charges == Some((3, 5)) && !known.tags.is_empty());
        assert!(unknown.unknown, "the row says it is unknown");
        assert_eq!((unknown.label.as_str(), unknown.goes_on.as_slice()), ("teak rod", ["hand".to_string()].as_slice()), "by its name and its slot");
        assert!(unknown.used.is_empty() && unknown.blow.is_none() && unknown.armor == 0, "nothing it does or strikes for");
        assert!(unknown.bestows.is_empty() && unknown.tags.is_empty() && unknown.charges.is_none(), "nor what it lends, counts as or holds");
        assert!(unknown.uses_something && !unknown.empty, "and its use is still offered");
    }

    /// An empty thing its user knows is not used from the bag, but an empty
    /// one not yet known keeps its use key, since a key gone would say it
    /// was empty.
    #[test]
    fn an_empty_thing_not_yet_known_keeps_its_use_key() {
        let mut stage = Stage::new(InventoryViewPlugin);
        let player = stage.player;
        let empty = || (Item, Triggers(vec![on_use(None)]), Consumable { left: 0, ..Consumable::new(3, WhenEmpty::Kept) });
        let known = stage.app.world_mut().spawn((empty(), Name::new("ash wand"))).id();
        let unknown = stage.app.world_mut().spawn((empty(), Name::new("teak wand"), rl_bevy::Unknown)).id();
        stage.app.world_mut().entity_mut(player).insert(Inventory { items: vec![known, unknown] });
        stage.tick();
        let view = stage.app.world().resource::<InventoryView>();
        assert!(view.rows[0].empty && !view.rows[0].usable(), "known, it is empty and not used");
        assert!(!view.rows[1].empty && view.rows[1].usable(), "not yet known, nothing says it is empty, and it is used");
    }

    /// A worn stuck thing's row says it will not come off, which is what
    /// the bag's keys read; in the bag it is listed as anything else, since
    /// only being worn holds it.
    #[test]
    fn a_stuck_things_row_says_so_only_while_it_is_worn() {
        let mut stage = Stage::new_with(InventoryViewPlugin, |app| {
            app.world_mut().resource_mut::<Registries>().slots = Registry::from_defs(vec![SlotDef::new("finger")]).unwrap();
        });
        let player = stage.player;
        let finger = EquipShape::in_slot(stage.app.world().resource::<Registries>().slots.expect("finger"));
        let ring = stage.app.world_mut().spawn((Item, Name::new("ring"), Wearable(finger.clone()), rl_bevy::Stuck)).id();
        stage.app.world_mut().entity_mut(player).insert((Inventory { items: vec![ring] }, Equipped(Equipment::with_slot_count(1))));
        stage.tick();
        assert!(!stage.app.world().resource::<InventoryView>().rows[0].stuck, "in the bag, it comes and goes");
        stage.app.world_mut().get_mut::<Equipped>(player).unwrap().equip(ring, &finger).unwrap();
        stage.tick();
        assert!(stage.app.world().resource::<InventoryView>().rows[0].stuck, "worn, it will not come off");
    }

    /// A thing whose use asks for another lists, under its prompt, what it
    /// may be made on, by the rule the items resolver takes a choice by:
    /// carried, not itself, and counted as what it asks for, worn or not.
    /// With nothing to choose, its use key does nothing.
    #[test]
    fn a_row_whose_use_asks_for_a_thing_lists_what_it_may_be_made_on() {
        let mut stage = Stage::new_with(InventoryViewPlugin, |app| {
            let mut registries = app.world_mut().resource_mut::<Registries>();
            registries.tags = Registry::from_defs(vec![rl_rules::TagDef::new("kit")]).unwrap();
            registries.slots = Registry::from_defs(vec![SlotDef::new("body")]).unwrap();
        });
        let player = stage.player;
        let (kit, body) = {
            let r = stage.app.world().resource::<Registries>();
            (r.tags.expect("kit"), EquipShape::in_slot(r.slots.expect("body")))
        };
        let asks = rl_rules::Asks { prompt: "Polish which?".to_string(), tag: Some(rl_rules::NameRef::from(kit)) };
        let polish = stage.app.world_mut().spawn((Item, Name::new("polish"), Tagged(vec![kit]), Triggers(vec![on_use(Some(asks))]))).id();
        let coat = stage.app.world_mut().spawn((Item, Name::new("coat"), Tagged(vec![kit]), Wearable(body.clone()))).id();
        let stone = stage.app.world_mut().spawn((Item, Name::new("stone"))).id();
        let boots = stage.app.world_mut().spawn((Item, Name::new("boots"), Tagged(vec![kit]))).id();
        let mut worn = Equipped(Equipment::with_slot_count(1));
        worn.equip(coat, &body).unwrap();
        stage.app.world_mut().entity_mut(player).insert((Inventory { items: vec![polish, coat, stone, boots] }, worn));
        stage.tick();
        let row = stage.app.world().resource::<InventoryView>().rows[0].clone();
        let asking = row.asks.clone().expect("its use asks for a thing");
        assert_eq!((asking.prompt.as_str(), asking.choices), ("Polish which?", vec![coat, boots]), "kit, worn or not, and never itself");
        assert!(row.usable());

        stage.app.world_mut().entity_mut(player).insert(Inventory { items: vec![polish, stone] });
        stage.tick();
        let row = &stage.app.world().resource::<InventoryView>().rows[0];
        assert!(row.asks.as_ref().is_some_and(|a| a.choices.is_empty()) && !row.usable(), "nothing to choose, nothing to use");
    }
}
