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

use atglyph_bevy::TriggerKindId;
use atglyph_bevy::prelude::*;
use atglyph_render::Glyph;
use atglyph_rules::SlotId;
use atglyph_rules::parts::ShieldDef;
use atglyph_rules::stats::Op;
use bevy::prelude::*;

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
    /// What it adds to what it launches while worn, for a launcher.
    pub launches: Option<LaunchLine>,
    /// How far it lets its wearer see in the dark, for a thing that lends
    /// sight, [`WornDarkSight`](atglyph_bevy::WornDarkSight).
    pub dark_sight: Option<i32>,
    /// What it blocks while worn, for a [`Shield`](atglyph_bevy::Shield).
    pub shield: Option<ShieldDef>,
    /// The percent it leaves its wearer's attacks at while worn, for a
    /// thing with an [`AttackSpeed`](atglyph_bevy::AttackSpeed): below a
    /// hundred is slower.
    pub attack_speed: Option<u32>,
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
    pub targeting: Option<atglyph_rules::Targeting>,
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
    pub bestows: Vec<Bestowed>,
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
    /// What putting it on would change, for a thing that can be worn and
    /// is not: where it would go and what would come off. `None` for a
    /// thing worn or that cannot be, and for a wearer with no
    /// [`Equipped`].
    pub replacing: Option<Replacing>,
}

/// What a thing does to one registered stat while worn, as a bag reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bestowed {
    /// The stat's registered name.
    pub stat: String,
    /// What it does to it.
    pub op: Op,
    /// Whether less of the stat is the better, its
    /// [`StatDef::lower_is_better`](atglyph_rules::StatDef::lower_is_better),
    /// so a screen weighing gear tones a fall as a gain.
    pub lower_is_better: bool,
}

/// What a launcher adds to what it launches, as a bag reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchLine {
    /// What it launches, by the tag's registered name.
    pub ammo: String,
    /// The cells it adds to how far a thing launched flies.
    pub range: u32,
    /// The roll it adds to what a thing launched strikes for.
    pub roll: atglyph_core::DiceRoll,
}

/// What putting a thing on would change, against what its wearer has on:
/// the slot it would go in, what would come off, and the slots left bare.
///
/// Worked out by [`Equipment::swap`](atglyph_rules::Equipment::swap), the
/// rule the items resolver equips by, so a screen never names a slot or a
/// thing that wearing it would not. Where it would push off a
/// [`Stuck`](atglyph_bevy::Stuck) thing, wearing it is refused; the row then says what it would take off
/// were that free, and the stuck thing's row says it is stuck.
#[derive(Debug, Clone, PartialEq)]
pub struct Replacing {
    /// The slot it would go in, by its registered name.
    pub slot: String,
    /// What would come off, in the order the thing's slots are taken.
    pub off: Vec<TakenOff>,
    /// The slots what comes off holds that the thing would not take, by
    /// their registered names, left bare: the hand a one-hander frees,
    /// put on in place of a two-hander.
    pub frees: Vec<String>,
    /// What the wearer strikes, shoots, launches and sees with now.
    pub before: Kit,
    /// What it would strike, shoot, launch and see with once the thing is
    /// on: decided by everything then worn, as a fight decides it, so a
    /// dagger put on in place of a bow is weighed against the bare hand
    /// the bow left, and a knife for the off hand against nothing, the
    /// sword in the other still striking.
    pub after: Kit,
}

/// What a wearer fights and sees with, by the rules a fight uses: the blow
/// and the shot of the first worn thing, in slot order, that has one and
/// a charge to use it, else the wearer's own; the first worn launcher for
/// each thing launched; and the best sight in the dark of its own and of
/// what it wears.
#[derive(Debug, Clone, PartialEq)]
pub struct Kit {
    /// The blow struck, and what with.
    pub blow: Option<Wielding>,
    /// The shot fired, and what with.
    pub shot: Option<Wielding>,
    /// The launcher for each thing launched, the first worn for it.
    pub launchers: Vec<LaunchLine>,
    /// How far the wearer sees in the dark.
    pub dark_sight: Option<i32>,
    /// What the wearer blocks with: the first worn shield, in slot order,
    /// the one a block is rolled for.
    pub shield: Option<ShieldDef>,
    /// The percent every blow, shot and throw is struck at: the wearer's
    /// own and every worn thing's, multiplied; a hundred for none.
    pub attack_speed: u32,
    /// Whether any of it comes from a thing not yet known, whose numbers a
    /// screen must not tell.
    pub withheld: bool,
}

impl Default for Kit {
    /// A wearer with nothing: no blow, shot, launcher, sight or shield, and
    /// attacks at an ordinary hundred percent rather than none.
    fn default() -> Self {
        Self { blow: None, shot: None, launchers: Vec::new(), dark_sight: None, shield: None, attack_speed: 100, withheld: false }
    }
}

/// A blow or a shot and what it comes from.
#[derive(Debug, Clone, PartialEq)]
pub struct Wielding {
    /// The worn thing it comes from, by its name, or `None` for the
    /// wearer's own.
    pub from: Option<String>,
    /// The blow or shot.
    pub strike: Strike,
}

/// A worn thing that putting another on would take off.
#[derive(Debug, Clone, PartialEq)]
pub struct TakenOff {
    /// It, described as the bag describes it.
    pub row: ItemRow,
    /// The slots of its the other would take, by their registered names:
    /// the slot it goes in, or only the hand a two-hander takes as well.
    pub from: Vec<String>,
}

/// What a use that asks for one of its user's things asks for, as a bag
/// reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asking {
    /// What the choice is asked under, in the game's words.
    pub prompt: String,
    /// What it may be made on, in the bag's order, by
    /// [`may_use_on`](atglyph_bevy::may_use_on), the rule the items resolver
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
            launches: None,
            dark_sight: None,
            shield: None,
            attack_speed: None,
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
        app.depends_on::<crate::UiPlugin>("InventoryViewPlugin");
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
    Option<&'static BonusDamage>,
    Option<&'static Bestows>,
);
/// What an item does at its moments, what a use costs it, its clock,
/// whether its charges come back only while worn, and its `EffectBonus`.
type Does = (
    Option<&'static Triggers>,
    Option<&'static Consumable>,
    Option<&'static atglyph_bevy::WornTimer>,
    Has<atglyph_bevy::Attuned>,
    Option<&'static atglyph_bevy::EffectBonus>,
);
/// Whether an item is not yet known, and whether it will not come off.
type Marks = (Has<atglyph_bevy::Unknown>, Has<atglyph_bevy::Stuck>);
/// What an item lends its wearer beyond a blow: a launcher's roll and
/// reach, sight in the dark, what it blocks and how it slows an attack.
type Lends = (
    Option<&'static atglyph_bevy::Launcher>,
    Option<&'static atglyph_bevy::WornDarkSight>,
    Option<&'static atglyph_bevy::Shield>,
    Option<&'static atglyph_bevy::AttackSpeed>,
);

/// How a trigger's lines are introduced on a bag's row: by what the player
/// does to set it off, in the engine's own moments, and by the moment's
/// name for one a game registered. A pulse says how often it comes round.
fn lead_in(moment: TriggerKindId, moments: Option<&TriggerKinds>, pulse: Option<&atglyph_bevy::WornTimer>) -> String {
    match moment {
        m if m == TriggerKinds::USE => "use".to_string(),
        // Not "thrown": the row already says how far it flies, and the
        // word twice over reads as two things.
        m if m == TriggerKinds::LAND => "on landing".to_string(),
        m if m == TriggerKinds::HIT => "on a hit".to_string(),
        m if m == TriggerKinds::FIRE => "when fired".to_string(),
        m if m == TriggerKinds::PULSE => match pulse {
            // "Every 1 turn" is a count nobody says aloud.
            Some(p) if p.every <= atglyph_core::turn::BASE_ACTION_COST => "every turn worn".to_string(),
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
fn describe_uses(
    triggers: &Triggers,
    registries: &Registries,
    moments: Option<&TriggerKinds>,
    pulse: Option<&atglyph_bevy::WornTimer>,
    bonus: EffectBonus,
) -> Vec<String> {
    let lines_of = |trigger: &atglyph_bevy::Trigger| -> Vec<String> {
        let area = match trigger.area {
            atglyph_rules::EffectArea::Here => String::new(),
            atglyph_rules::EffectArea::Burst { radius } => format!(" in a burst of {radius}"),
        };
        trigger.effects.describe(registries, bonus).into_iter().map(|line| format!("{line}{area}")).collect()
    };
    // The one trigger on `moment`, and `None` for none or for more than
    // one: a second of either takes every one of its moment out of the
    // merge below, since there is no longer a single pair to fold.
    let only_on = |moment: TriggerKindId| {
        let mut on = triggers.on(moment);
        on.next().filter(|_| on.next().is_none())
    };
    // The shared lines, when the carrier's one `use` trigger and one
    // `land` trigger both carry something and say the same thing; `None`
    // leaves every trigger under its own name, including a `use` or a
    // `land` alone.
    let shared = match (only_on(TriggerKinds::USE), only_on(TriggerKinds::LAND)) {
        (Some(use_trigger), Some(land_trigger)) => {
            let (use_lines, land_lines) = (lines_of(use_trigger), lines_of(land_trigger));
            (!use_lines.is_empty() && use_lines == land_lines).then_some(use_lines)
        }
        _ => None,
    };
    let mut used = Vec::new();
    for trigger in &triggers.0 {
        if shared.is_some() && trigger.on == TriggerKinds::LAND {
            continue; // said once already, under the `use` trigger below.
        }
        match (&shared, trigger.on == TriggerKinds::USE) {
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
pub(crate) fn collect_inventory(mut view: ResMut<InventoryView>, player: Query<(Entity, &Inventory, Option<&Equipped>), With<Player>>, items: ItemRows) {
    view.rows.clear();
    view.entity = None;
    let Ok((entity, bag, worn)) = player.single() else { return };
    view.entity = Some(entity);
    view.rows.extend(bag.items.iter().filter_map(|item| items.row(entity, bag, worn, *item)));
}

/// Describes a thing as a bag row reads it, for whoever would carry or
/// wear it: the bag's own rows, and the trade screen's on both sides, so
/// a thing reads the same on a counter as in the pack.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct ItemRows<'w, 's> {
    registries: Option<Res<'w, Registries>>,
    moments: Option<Res<'w, TriggerKinds>>,
    items: Query<'w, 's, (Looks, Arms, Does, Marks, Lends), With<Item>>,
    tags: Query<'w, 's, &'static Tagged>,
    throws: atglyph_bevy::Throws<'w, 's>,
    wearers: Query<'w, 's, Own, Without<Item>>,
}

/// What a wearer fights and sees with of its own.
type Own = (
    Option<&'static MeleeAttack>,
    Option<&'static RangedAttack>,
    Option<&'static atglyph_bevy::OwnDarkSight>,
    Has<atglyph_bevy::DarkSightSuppressed>,
    Option<&'static atglyph_bevy::AttackSpeed>,
);

impl ItemRows<'_, '_> {
    /// `item` as `holder` reads it, carrying `bag` and wearing `worn`: how
    /// far it would throw it, what a use of it may be made on among `bag`,
    /// where it is worn, and, when it is not worn and could be, what
    /// putting it on would change. `None` for a thing that is not an
    /// [`Item`].
    pub(crate) fn row(&self, holder: Entity, bag: &Inventory, worn: Option<&Equipped>, item: Entity) -> Option<ItemRow> {
        let mut row = self.plain(holder, bag, worn, item)?;
        let (Some(worn), Some(shape)) = (worn, self.items.get(item).ok().and_then(|((_, _, _, wearable, _, _), ..)| wearable)) else { return Some(row) };
        if row.worn() {
            return Some(row);
        }
        // What wearing it would take off, by the rule the resolver equips
        // by. Where a stuck thing refuses it, what it would take off were
        // that free, so the row can say what stands in the way.
        let stuck = |thing: Entity| self.items.get(thing).is_ok_and(|(.., (_, stuck), _)| stuck);
        let swap = worn.0.swap(item, &shape.0, stuck).or_else(|_| worn.0.swap(item, &shape.0, |_| false));
        let slot_name = |slot| self.registries.as_deref().map(|r| r.slots.name(slot).to_string()).unwrap_or_default();
        let before = self.kit(holder, &worn.0);
        row.replacing = swap.ok().map(|swap| Replacing {
            before,
            after: self.kit(holder, &swap.after),
            slot: slot_name(swap.slot),
            off: swap
                .off
                .iter()
                .filter_map(|(thing, from)| {
                    Some(TakenOff { row: self.plain(holder, bag, Some(worn), *thing)?, from: from.iter().map(|s| slot_name(*s)).collect() })
                })
                .collect(),
            frees: swap.frees.iter().map(|s| slot_name(*s)).collect(),
        });
        Some(row)
    }

    /// What `wearer` fights and sees with, wearing `worn`: see [`Kit`].
    fn kit(&self, wearer: Entity, worn: &atglyph_rules::Equipment<Entity>) -> Kit {
        let (own_blow, own_shot, own_sight, blind, own_speed) = self.wearers.get(wearer).unwrap_or((None, None, None, false, None));
        let registries = self.registries.as_deref();
        let kind_name = |kind| registries.map(|r| r.damage_kinds.name(kind).to_string()).unwrap_or_default();
        let things: Vec<Entity> = worn.worn().map(|(_, thing)| thing).collect();
        let of = |thing: Entity| self.items.get(thing).ok();
        let charged = |thing: Entity| !of(thing).is_some_and(|(_, _, (_, consumable, ..), _, _)| consumable.is_some_and(|c| c.is_empty()));
        let unknown = |thing: Option<Entity>| thing.and_then(of).is_some_and(|(_, _, _, (unknown, _), _)| unknown);
        let name = |thing: Entity| of(thing).and_then(|((name, ..), ..)| name).map(|n| n.as_str().to_string()).unwrap_or_default();
        let blow = atglyph_rules::arms::first_charged(
            things.iter().map(|t| (*t, of(*t).and_then(|(_, (_, _, m, ..), ..)| m.copied()), charged(*t))),
            own_blow.copied(),
        );
        let shot = atglyph_rules::arms::first_charged(
            things.iter().map(|t| (*t, of(*t).and_then(|(_, (_, _, _, r, ..), ..)| r.copied()), charged(*t))),
            own_shot.copied(),
        );
        let mut withheld = unknown(blow.and_then(|(from, _)| from)) || unknown(shot.and_then(|(from, _)| from));
        let wielding =
            |from: Option<Entity>, kind, dice, range| Wielding { from: from.map(name), strike: Strike { kind: kind_name(kind), dice, range, also: None } };
        let mut launchers: Vec<LaunchLine> = Vec::new();
        let mut seen = Vec::new();
        for thing in &things {
            if let Some((_, _, _, (unknown, _), (Some(launcher), ..))) = of(*thing)
                && !seen.contains(&launcher.ammo)
            {
                seen.push(launcher.ammo);
                withheld |= unknown;
                let ammo = registries.map(|r| r.tags.name(launcher.ammo).to_string()).unwrap_or_default();
                launchers.push(LaunchLine { ammo, range: launcher.range, roll: launcher.damage });
            }
        }
        let lent: Vec<(i32, bool)> =
            things.iter().filter_map(|t| of(*t).and_then(|(_, _, _, (unknown, _), (_, sight, ..))| sight.map(|s| (s.0, unknown)))).collect();
        withheld |= lent.iter().any(|(_, unknown)| *unknown);
        // The first shield worn is the one a block is rolled for, and every
        // attack speed worn is multiplied in, so a thing not yet known
        // among either keeps the numbers back.
        let shielded = things.iter().find_map(|t| of(*t).and_then(|(.., (unknown, _), (_, _, shield, _))| shield.map(|s| (s.0, unknown))));
        let slowed: Vec<(atglyph_bevy::AttackSpeed, bool)> =
            things.iter().filter_map(|t| of(*t).and_then(|(.., (unknown, _), (.., speed))| speed.map(|s| (*s, unknown)))).collect();
        withheld |= shielded.is_some_and(|(_, unknown)| unknown) || slowed.iter().any(|(_, unknown)| *unknown);
        Kit {
            blow: blow.map(|(from, m)| wielding(from, m.kind, m.dice, None)),
            shot: shot.map(|(from, r)| wielding(from, r.kind, r.dice, Some(r.range))),
            launchers,
            dark_sight: atglyph_rules::equip::dark_sight(own_sight.map(|s| s.0), lent.iter().map(|(tiles, _)| *tiles), blind),
            shield: shielded.map(|(shield, _)| shield),
            attack_speed: atglyph_bevy::AttackSpeed::combined(own_speed.copied().into_iter().chain(slowed.iter().map(|(s, _)| *s))),
            withheld,
        }
    }

    /// `item` described with no word on what putting it on would change.
    fn plain(&self, holder: Entity, bag: &Inventory, worn: Option<&Equipped>, item: Entity) -> Option<ItemRow> {
        let (
            (name, glyph, stack, wearable, _, tagged),
            (armor, resists, melee, ranged, strikes, bestows),
            (triggers, consumable, pulse, attuned, bonus),
            (unknown, stuck),
            (launcher, sight, shield, speed),
        ) = self.items.get(item).ok()?;
        let registries = self.registries.as_deref();
        let moments = self.moments.as_deref();
        let kind_name = |kind| registries.map(|r| r.damage_kinds.name(kind).to_string()).unwrap_or_default();
        let slot_name = |slot| registries.map(|r| r.slots.name(slot).to_string()).unwrap_or_default();
        let tag_name = |tag| registries.map(|r| r.tags.name(tag).to_string()).unwrap_or_default();
        let strike = |(kind, dice): (atglyph_rules::damage::DamageKindId, atglyph_core::DiceRoll), range: Option<i32>| Strike {
            kind: kind_name(kind),
            dice,
            range,
            also: None,
        };
        let slot = worn.and_then(|w| w.slot_of(item));
        // How it flies from its holder, its own range or the holder's reach.
        let thrown = self.throws.of(holder, item);
        // What its use may be made on, by the rule the resolver takes a
        // choice by, among the things the bag lists.
        let asks = triggers.and_then(Triggers::asks).map(|asks| Asking {
            prompt: asks.prompt.clone(),
            choices: bag
                .items
                .iter()
                .copied()
                .filter(|on| self.items.contains(*on) && atglyph_bevy::may_use_on(asks, item, *on, bag, self.tags.get(*on).ok()))
                .collect(),
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
            .map(|r| (r.every - r.progress.min(r.every)).div_ceil(atglyph_core::turn::BASE_ACTION_COST));
        let row = ItemRow {
            entity: item,
            used,
            uses_something: triggers.is_some_and(|t| t.on(TriggerKinds::USE).next().is_some()),
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
            throw_range: thrown.map(|t| t.range),
            thrown: thrown.and_then(|t| t.strike.map(|s| Strike { also: t.launched.map(|(_, roll)| roll), ..strike(s, Some(t.range)) })),
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
            launches: launcher.map(|l| LaunchLine { ammo: tag_name(l.ammo), range: l.range, roll: l.damage }),
            dark_sight: sight.map(|s| s.0),
            shield: shield.map(|s| s.0),
            attack_speed: speed.map(|s| s.0),
            bestows: bestows
                .map(|b| {
                    b.0.iter()
                        .map(|(stat, op)| Bestowed {
                            stat: registries.map(|r| r.stats.name(*stat).to_string()).unwrap_or_default(),
                            op: *op,
                            lower_is_better: registries.is_some_and(|r| r.stats.get(*stat).lower_is_better),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            tags: tagged.map(|t| t.0.iter().map(|tag| tag_name(*tag)).collect()).unwrap_or_default(),
            facets: Vec::new(),
            seems: Vec::new(),
            ready_in,
            attuned,
            unknown: false,
            stuck: stuck && slot.is_some(),
            replacing: None,
        };
        Some(if unknown { row.withheld() } else { row })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;
    use atglyph_core::DiceRoll;
    use atglyph_rules::content::Registry;
    use atglyph_rules::{EquipShape, Equipment, SlotDef, StatDef};

    /// A kit of nothing attacks at an ordinary hundred percent.
    #[test]
    fn a_default_kit_attacks_at_an_ordinary_speed() {
        assert_eq!(Kit::default().attack_speed, 100);
    }

    /// A shield is listed with what it blocks and a slowing thing with the
    /// percent it leaves attacks at, and a thing not yet known with neither.
    #[test]
    fn a_shield_row_carries_what_it_blocks_and_the_speed_it_leaves() {
        let mut stage = Stage::new(InventoryViewPlugin);
        let player = stage.player;
        let w = stage.app.world_mut();
        let def = atglyph_rules::parts::ShieldDef { chance: 50, blocks: 2 };
        let kite = w.spawn((Item, Name::new("kite"), atglyph_bevy::Shield(def), atglyph_bevy::AttackSpeed(90))).id();
        let hidden = w.spawn((Item, Name::new("kite"), atglyph_bevy::Shield(def), atglyph_bevy::AttackSpeed(90), atglyph_bevy::Unknown)).id();
        let plain = w.spawn((Item, Name::new("rag"))).id();
        w.entity_mut(player).insert(Inventory { items: vec![kite, hidden, plain] });
        stage.tick();
        let rows = &stage.app.world().resource::<InventoryView>().rows;
        assert_eq!((rows[0].shield, rows[0].attack_speed), (Some(def), Some(90)));
        assert_eq!((rows[1].shield, rows[1].attack_speed), (None, None), "withheld while unknown");
        assert_eq!((rows[2].shield, rows[2].attack_speed), (None, None));
    }

    /// A thing with no range of its own is listed as flying as far as the
    /// player reaches.
    #[test]
    fn a_rangeless_throw_is_listed_at_the_players_reach() {
        let mut stage = Stage::new(InventoryViewPlugin);
        let (player, kind) = (stage.player, stage.kind);
        stage.app.world_mut().resource_mut::<atglyph_bevy::CombatRules>().reach = Some(atglyph_bevy::ThrowReach { base: 4, stat: None });
        let stone = stage.app.world_mut().spawn((Item, Name::new("stone"), Throwable::new(None, Some((kind, DiceRoll::new(1, 3)))))).id();
        stage.app.world_mut().entity_mut(player).insert(Inventory { items: vec![stone] });
        stage.tick();
        let row = &stage.app.world().resource::<InventoryView>().rows[0];
        assert_eq!(row.throw_range, Some(4));
        assert_eq!(row.thrown.as_ref().map(|s| s.range), Some(Some(4)));
    }

    /// A thing a worn launcher launches is listed at the reach and the
    /// launcher's range together, its strike with the launcher's roll.
    #[test]
    fn a_launched_throw_is_listed_with_the_launchers_range_and_roll() {
        let mut stage = Stage::new_with(InventoryViewPlugin, |app| {
            let mut registries = app.world_mut().resource_mut::<Registries>();
            registries.slots = Registry::from_defs(vec![SlotDef::new("hand")]).unwrap();
            registries.tags = Registry::from_defs(vec![atglyph_rules::TagDef::new("pellet")]).unwrap();
        });
        let (player, kind) = (stage.player, stage.kind);
        stage.app.world_mut().resource_mut::<atglyph_bevy::CombatRules>().reach = Some(atglyph_bevy::ThrowReach { base: 4, stat: None });
        let ammo = atglyph_rules::TagId::from_raw(0);
        let shape = EquipShape::in_slot(atglyph_rules::SlotId::from_raw(0));
        let launcher = atglyph_bevy::Launcher { ammo, range: 3, damage: DiceRoll::new(1, 4) };
        let launcher_item = stage.app.world_mut().spawn((Item, Name::new("launcher"), launcher, Wearable(shape.clone()))).id();
        let pellet = stage.app.world_mut().spawn((Item, Name::new("pellet"), Tagged(vec![ammo]), Throwable::new(None, Some((kind, DiceRoll::new(1, 2)))))).id();
        let mut worn = Equipment::with_slot_count(1);
        worn.equip(launcher_item, &shape).unwrap();
        stage.app.world_mut().entity_mut(player).insert((Inventory { items: vec![launcher_item, pellet] }, Equipped(worn)));
        stage.tick();
        let row = &stage.app.world().resource::<InventoryView>().rows[1];
        assert_eq!(row.throw_range, Some(7));
        assert_eq!(row.thrown.as_ref().map(|s| (s.dice, s.also)), Some((DiceRoll::new(1, 2), Some(DiceRoll::new(1, 4)))));
    }

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
                BonusDamage(vec![(kind, DiceRoll::flat(1))]),
                Bestows(vec![(might, Op::Add(2))]),
            ))
            .id();
        let mut resists = atglyph_rules::Resistances::new();
        resists.set(kind, 10);
        let hat = stage.app.world_mut().spawn((Item, Name::new("a hat"), Wearable(EquipShape::in_slot(head)), Armor(1), Resists(resists))).id();
        let knives = stage
            .app
            .world_mut()
            .spawn((Item, Name::new("knife"), Stack { key: 1, count: 3 }, Throwable::new(Some(5), Some((kind, DiceRoll::new(1, 4))))))
            .id();
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
        assert_eq!(blade.bestows, vec![Bestowed { stat: "might".to_string(), op: Op::Add(2), lower_is_better: false }], "by the stat's name");

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
        let on_use = atglyph_bevy::Trigger {
            on: atglyph_bevy::TriggerKinds::USE,
            area: atglyph_rules::EffectArea::Here,
            fires: None,
            effects: std::sync::Arc::new(atglyph_bevy::Effects::default()),
            look: None,
            every: None,
            targeting: None,
            asks: None,
        };
        let plate = stage.app.world_mut().spawn((Item, Name::new("plate"), Wearable(EquipShape::in_slot(torso)), atglyph_bevy::Triggers(vec![on_use]))).id();
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
        assert_eq!(lead_in(TriggerKinds::PULSE, None, Some(&atglyph_bevy::WornTimer::every(800))), "every 8 turns worn");
        assert_eq!(
            lead_in(TriggerKinds::PULSE, None, Some(&atglyph_bevy::WornTimer::every(750))),
            "every 8 turns worn",
            "a part turn rounds up, never promising early"
        );
        assert_eq!(lead_in(TriggerKinds::PULSE, None, Some(&atglyph_bevy::WornTimer::every(100))), "every turn worn", "one turn is no count at all");
        assert_eq!(lead_in(TriggerKinds::PULSE, None, Some(&atglyph_bevy::WornTimer::every(50))), "every turn worn", "nor is less than one");
    }

    /// A row's effects are described with its item's own `EffectBonus`
    /// added: a plain `Inflict` of ten turns and a carrier bonus of four
    /// reads fourteen.
    #[test]
    fn a_rows_effects_are_described_with_its_effectbonus_added() {
        let mut stage = Stage::new_with(InventoryViewPlugin, |app| {
            app.world_mut().resource_mut::<Registries>().statuses = Registry::from_defs(vec![atglyph_rules::StatusDef::new("cloaked")]).unwrap();
        });
        let player = stage.player;
        let registries = stage.app.world().resource::<Registries>().clone();
        let mut kinds = EffectKinds::default();
        kinds.declare::<atglyph_bevy::Inflict>();
        let args = atglyph_rules::ability::parse_args(r#"(status: "cloaked", turns: 10)"#).unwrap();
        let specs = vec![atglyph_rules::EffectSpec { kind: "Inflict".to_string(), chance: 100, args }];
        let effects = Effects::build(&specs, &kinds, &registries.names()).unwrap();
        let trigger = atglyph_bevy::Trigger {
            on: TriggerKinds::USE,
            area: atglyph_rules::EffectArea::Here,
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
    fn plain_trigger(on: TriggerKindId, effects: &std::sync::Arc<Effects>) -> atglyph_bevy::Trigger {
        atglyph_bevy::Trigger {
            on,
            area: atglyph_rules::EffectArea::Here,
            fires: None,
            effects: effects.clone(),
            look: None,
            every: None,
            targeting: None,
            asks: None,
        }
    }

    /// A `Mend` effect list of one roll, built the way an item's would be.
    fn mend(registries: &Registries, kinds: &EffectKinds, roll: &str) -> std::sync::Arc<Effects> {
        let args = atglyph_rules::ability::parse_args(&format!(r#"(roll: "{roll}")"#)).unwrap();
        let specs = vec![atglyph_rules::EffectSpec { kind: "Mend".to_string(), chance: 100, args }];
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
        kinds.declare::<atglyph_bevy::Mend>();
        let (shared, different) = (mend(&registries, &kinds, "3d6+4"), mend(&registries, &kinds, "1d4"));
        let ember = stage
            .app
            .world_mut()
            .spawn((Item, Name::new("ember"), Triggers(vec![plain_trigger(TriggerKinds::USE, &shared), plain_trigger(TriggerKinds::LAND, &shared)])))
            .id();
        let odd = stage
            .app
            .world_mut()
            .spawn((Item, Name::new("pebble"), Triggers(vec![plain_trigger(TriggerKinds::USE, &different), plain_trigger(TriggerKinds::LAND, &shared)])))
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
        kinds.declare::<atglyph_bevy::Mend>();
        let (shared, different) = (mend(&registries, &kinds, "3d6+4"), mend(&registries, &kinds, "1d4"));
        let ember = stage
            .app
            .world_mut()
            .spawn((
                Item,
                Name::new("ember"),
                Triggers(vec![
                    plain_trigger(TriggerKinds::USE, &shared),
                    plain_trigger(TriggerKinds::USE, &different),
                    plain_trigger(TriggerKinds::LAND, &shared),
                ]),
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
        kinds.declare::<atglyph_bevy::Mend>();
        let (shared, different) = (mend(&registries, &kinds, "3d6+4"), mend(&registries, &kinds, "1d4"));
        let ember = stage
            .app
            .world_mut()
            .spawn((
                Item,
                Name::new("ember"),
                Triggers(vec![
                    plain_trigger(TriggerKinds::USE, &shared),
                    plain_trigger(TriggerKinds::LAND, &shared),
                    plain_trigger(TriggerKinds::LAND, &different),
                ]),
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
    fn on_use(asks: Option<atglyph_rules::Asks>) -> atglyph_bevy::Trigger {
        let effects = std::sync::Arc::new(atglyph_bevy::Effects::default());
        atglyph_bevy::Trigger {
            on: TriggerKinds::USE,
            area: atglyph_rules::EffectArea::Here,
            fires: None,
            effects,
            look: None,
            every: None,
            targeting: None,
            asks,
        }
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
            registries.statuses = Registry::from_defs(vec![atglyph_rules::StatusDef::new("cloaked")]).unwrap();
            registries.tags = Registry::from_defs(vec![atglyph_rules::TagDef::new("kit")]).unwrap();
        });
        let (player, kind) = (stage.player, stage.kind);
        let registries = stage.app.world().resource::<Registries>().clone();
        let (hand, might, kit) = (registries.slots.expect("hand"), registries.stats.expect("might"), registries.tags.expect("kit"));
        let mut kinds = EffectKinds::default();
        kinds.declare::<atglyph_bevy::Inflict>();
        let args = atglyph_rules::ability::parse_args(r#"(status: "cloaked", turns: 10)"#).unwrap();
        let specs = vec![atglyph_rules::EffectSpec { kind: "Inflict".to_string(), chance: 100, args }];
        let cloak = atglyph_bevy::Trigger { effects: std::sync::Arc::new(Effects::build(&specs, &kinds, &registries.names()).unwrap()), ..on_use(None) };
        let rod = |name: &str| {
            (
                (Item, Name::new(name.to_string()), Wearable(EquipShape::in_slot(hand)), Tagged(vec![kit])),
                (MeleeAttack::new(kind, DiceRoll::new(1, 6)), Armor(2), Bestows(vec![(might, Op::Add(2))])),
                (Triggers(vec![cloak.clone()]), Consumable { left: 3, ..Consumable::new(5, WhenEmpty::Kept) }),
            )
        };
        let known = stage.app.world_mut().spawn(rod("ash rod")).id();
        let unknown = stage.app.world_mut().spawn((rod("teak rod"), atglyph_bevy::Unknown)).id();
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
        let unknown = stage.app.world_mut().spawn((empty(), Name::new("teak wand"), atglyph_bevy::Unknown)).id();
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
        let ring = stage.app.world_mut().spawn((Item, Name::new("ring"), Wearable(finger.clone()), atglyph_bevy::Stuck)).id();
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
            registries.tags = Registry::from_defs(vec![atglyph_rules::TagDef::new("kit")]).unwrap();
            registries.slots = Registry::from_defs(vec![SlotDef::new("body")]).unwrap();
        });
        let player = stage.player;
        let (kit, body) = {
            let r = stage.app.world().resource::<Registries>();
            (r.tags.expect("kit"), EquipShape::in_slot(r.slots.expect("body")))
        };
        let asks = atglyph_rules::Asks { prompt: "Polish which?".to_string(), tag: Some(atglyph_rules::NameRef::from(kit)) };
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
