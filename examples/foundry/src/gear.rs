//! Gear: the weapons, the armor, the slugs, the medical pair and the
//! grenades that arm the foundry's decks, loaded once from `items.ron` into
//! the [`Armory`] resource, found where `item_spawns.ron` says, and spawned
//! as items an actor can find, carry, wear and use.
//!
//! The [`Armory`] is Foundry's [`ItemMaker`]: the engine's `LootPlugin`
//! decides what lies on a deck, what a kill leaves and what a crate holds,
//! and asks the armory to make each thing, rolling a level from
//! `levels.ron` at the band it is found at for a thing that names a
//! `level`: a found plate is better the deeper it lay.
//!
//! What a level does to a thing is written in one place, its `level`, the
//! engine's [`Level`] part: the item's own numbers are what it is at `+0`,
//! and the level block says what each level adds to them, with Foundry's
//! own rule under it, [`level_of`], that armor and damage grow by one a
//! level unless the block says otherwise. Nothing a trigger or an effect
//! carries knows about levels; the level reaches an effect as an
//! [`EffectBonus`] on the thing, written by the engine when it is spawned.
//!
//! An [`ItemDef`] is the file's own vocabulary: everything a game needs
//! to know about a weapon or a suit of plate, named rather than typed, so
//! a new weapon is a line in the file and nothing here changes. Where a
//! field is something the engine plays, it is typed with the engine's
//! authored part and built once into the thing's [`ItemParts`], which the
//! engine spawns it from. What wielding or wearing one does is read
//! straight off the spawned entity by the engine's own combat and equip
//! machinery; nothing is copied onto whoever carries it.
//!
//! [`Heat`] is Foundry's own; `Ammo` is the engine's, lifted out of this
//! game into `rl_bevy::ammo` once a second game needed the same loop. This
//! module only attaches whichever one a weapon's entry names to it, so
//! neither module's own behaviour touches the loader.

use std::sync::Arc;

use bevy::prelude::*;
use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_bevy::{ItemMaker, LootArea, Provenance};
use rl_engine::rl_core::Id;
use rl_engine::rl_render::Glyph;
use rl_engine::rl_rules::parts::{self, Level};
use rl_engine::rl_rules::{DamageKind, LevelTable, LootTable, NameRef, Named, Registry, ScatterRules, TagDef};
use rl_engine::rl_rules::{EffectSpec, TriggerSpec};
use rl_engine::rl_ui::view::Strike;
use rl_engine::rl_ui::{Facets, InventoryView, MassNoun, Tones};
use serde::Deserialize;

use crate::heat::Heat;

const ITEMS_RON: &str = include_str!("../assets/items.ron");
const ITEM_SPAWNS_RON: &str = include_str!("../assets/item_spawns.ron");
const LEVELS_RON: &str = include_str!("../assets/levels.ron");

/// One kind of item, as authored in `items.ron`.
///
/// Unknown fields are refused, so a level rule left where an older file
/// wrote it, an `enchant` block or a top-level `pulse`, fails the load
/// naming it rather than loading a plate that quietly does less.
///
/// Every field the engine plays is one of its authored parts, a wear, a
/// blow, charges, a level, which the armory builds into the thing's
/// [`ItemParts`] once; the rest is Foundry's: a name and a look, heat and
/// dark sight.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemDef {
    /// Unique; spawn tables and, later, drop tables refer to it.
    pub name: String,
    /// Whether the name is said with no `a`, as a pair named in the plural
    /// is: `combat gauntlets`. The engine's narrator reads it as
    /// [`MassNoun`] on the thing.
    #[serde(default)]
    pub mass: bool,
    /// One character.
    pub glyph: char,
    /// `(r, g, b)` in `0..=1`.
    pub color: (f32, f32, f32),
    /// Where it is worn, if it is worn at all: a one-handed weapon either
    /// hand, so two can be dual wielded, a two-hander both.
    #[serde(default)]
    pub wear: Option<parts::Wear>,
    /// What it counts as: a weapon, armor, or a slug, for abilities and
    /// drops that ask by tag rather than by name.
    #[serde(default)]
    pub tags: Vec<NameRef<TagDef>>,
    /// What the thing holds, landed by any of its triggers that names no
    /// list of its own: written once, delivered by each.
    #[serde(default)]
    pub effects: Vec<EffectSpec>,
    /// What it does at its moments: used, thrown and landed, fired, hit.
    #[serde(default)]
    pub triggers: Vec<TriggerSpec>,
    /// What a use, a landing or a shot costs it, and how it refills;
    /// absent, nothing, which is what a tool is.
    #[serde(default)]
    pub consumable: Option<parts::Charges>,
    /// Flat armor while worn.
    #[serde(default)]
    pub armor: i32,
    /// Percent removed per damage kind, while worn.
    #[serde(default)]
    pub resists: Vec<(NameRef<DamageKind>, i32)>,
    /// The roll and damage kind a blow deals, while wielded.
    #[serde(default)]
    pub melee: Option<parts::Melee>,
    /// The range, roll and damage kind a shot deals, while wielded.
    #[serde(default)]
    pub ranged: Option<parts::Ranged>,
    /// How far it flies when thrown, and the blow it strikes whoever it
    /// hits, if any; absent, it cannot be thrown at all.
    #[serde(default)]
    pub throw: Option<parts::Throw>,
    /// What one blow or shot costs, in hundredths of a step. Absent is
    /// [`BASE_ACTION_COST`](rl_engine::rl_core::turn::BASE_ACTION_COST).
    #[serde(default)]
    pub cost: Option<u32>,
    /// Heat one shot adds, and heat a quiet turn sheds, for a weapon that
    /// runs hot rather than on ammunition.
    #[serde(default)]
    pub heat: Option<(u32, u32)>,
    /// The tag one shot spends, for a weapon that runs on ammunition
    /// rather than on heat.
    #[serde(default)]
    pub ammo: Option<NameRef<TagDef>>,
    /// Tiles seen without light while worn, at `+0`: the engine's worn
    /// dark sight, which its `level` grows by its own `dark_sight`.
    #[serde(default)]
    pub dark_sight: Option<i32>,
    /// That it takes levels, how far, and what a level adds: the one place
    /// a reader looks to learn what a `+2` is. Absent, it is always plain.
    ///
    /// The engine's level part, with Foundry's own rules on it: `max` is
    /// written, since it is the highest a thing is found at; armor and
    /// damage are inferred, +1 a level on a thing that has any, because
    /// that is what a reader expects of a `+2` plate or blade; everything
    /// else is nought unless written, because a reader cannot tell a
    /// plate's clock quickens unless the file says so. Balance is set by
    /// `max`, not by a slower rate.
    #[serde(default)]
    pub level: Option<Level>,
    /// Whether copies merge into one counted entry rather than one item
    /// each.
    #[serde(default)]
    pub stack: bool,
}

/// Whether `def` makes an attack roll of its own a level could add to: a
/// blow, a shot, or a strike when thrown.
fn attacks(def: &ItemDef) -> bool {
    def.melee.is_some() || def.ranged.is_some() || def.throw.is_some_and(|t| t.strike.is_some())
}

/// What a level of `def` adds, its line's `level` written over Foundry's
/// inferred armor and damage: +1 armor a level on a thing with armor of
/// its own and +1 damage a level on a thing that attacks, unless the line
/// says otherwise. `None` for a thing that takes no levels.
pub fn level_of(def: &ItemDef) -> Option<Level> {
    let inferred = Level { armor: (def.armor != 0).then_some(1), damage: attacks(def).then_some(1), ..Level::default() };
    def.level.as_ref().map(|own| own.over(&inferred))
}

/// `name` as a thing at `level` is called: `cloak plate +2`, and plain at
/// nought, since a `+0` is what every plain thing is.
pub fn named_at(name: &str, level: i32) -> String {
    if level == 0 { name.to_string() } else { format!("{name} {level:+}") }
}

impl Named for ItemDef {
    fn name(&self) -> &str {
        &self.name
    }
}

/// Notes on each pack row what the engine's row cannot read off the
/// thing: how far it sees in the dark, and the attack a weapon has put by
/// while it cannot use it, with why, `dry` of ammunition or `locked` by
/// its heat.
///
/// The engine reads a row's blow and shot off `MeleeAttack` and
/// `RangedAttack`, which a weapon that cannot fire has moved into the
/// engine's own `Stowed` so the loadout finds nothing, and without this a
/// slug rifle carried with no slugs, `+2` or not, reads as nothing but
/// where it goes. Its numbers go back in the row's own `blow` or `shot`,
/// so the pack says them in its own words, and the reason in a facet
/// under them.
///
/// Why it is put by is asked of the engine's `Loadout::dry`, the one
/// place that says whether ammunition benched a weapon, rather than read
/// off the marker here.
///
/// `InventoryView` is `None` until a game adds a pack panel: absence means
/// no pack to annotate, not that anything is wrong.
pub fn note_the_pack(
    view: Option<ResMut<InventoryView>>,
    mut facets: ResMut<Facets>,
    registries: Res<Registries>,
    loadout: Loadout,
    things: Query<(Option<&WornDarkSight>, Option<&Stowed>)>,
) {
    let Some(mut view) = view else { return };
    let kind = |kind| registries.damage_kinds.name(kind).to_string();
    for row in view.rows.iter_mut() {
        let Ok((sight, stowed)) = things.get(row.entity) else { continue };
        let dry = loadout.dry(row.entity);
        if let Some(stowed) = stowed {
            match *stowed {
                Stowed::Melee(a) => row.blow = Some(Strike { kind: kind(a.kind), dice: a.dice, range: None }),
                Stowed::Ranged(a) => row.shot = Some(Strike { kind: kind(a.kind), dice: a.dice, range: Some(a.range) }),
            }
            let why = if dry { facets.facet("ammo", "dry") } else { facets.facet("heat", "locked") };
            row.facets.push(why.toned(Tones::BAD));
        }
        if let Some(sight) = sight {
            row.facets.push(facets.facet("dark sight", format!("sees {} in the dark", sight.0)));
        }
    }
}

/// The item definitions and the table of what lies on which deck, loaded
/// once, in `PreStartup`, by [`load_armory`].
///
/// `Clone` but not `Debug`: what an item lands when it is used is a list of
/// boxed effects, shared by handle, and a trait object has nothing to print.
#[derive(Resource, Clone)]
pub struct Armory {
    /// The item definitions, by id.
    pub defs: Registry<ItemDef>,
    /// What can be found on a deck, drawn by band, where the band is the
    /// deck number: `item_spawns.ron`, through the engine's loader.
    pub table: LootTable<Id<ItemDef>>,
    /// How good a found thing is at each deck, for the things that name a
    /// `level`: `levels.ron`.
    pub levels: LevelTable,
    /// Each definition's parts, built once and shared by every item
    /// spawned from it: its triggers, its wear and its level among them.
    ///
    /// Built here rather than per item because an effect is a boxed trait
    /// object read out of RON: parsing a medkit's gel once a medkit is
    /// parsing it for every medkit on ten decks.
    parts: Vec<Arc<ItemParts>>,
}

/// One item's own shape, checked against nothing but itself: whether it
/// names a weapon that runs on both of the two economies at once, which
/// neither `heat_on_struck` nor `spend_ammo` is written to expect on the
/// same entity, and Foundry's own rules for a level. What the engine would
/// never play, a worn stack or a pulse on a thing never worn, the engine's
/// own check of the parts says when the armory builds them.
fn validate_def(d: &ItemDef, _: &Registry<ItemDef>) -> Result<(), String> {
    if d.heat.is_some() && d.ammo.is_some() {
        return Err("a weapon cannot run on both heat and ammo".into());
    }
    if d.dark_sight.is_some() && d.wear.is_none() {
        return Err("dark sight, on a thing that is never worn".into());
    }
    if let Some(level) = &d.level {
        validate_level(d, level)?;
    }
    Ok(())
}

/// The level's own rules, Foundry's: each key is refused where nothing on
/// the thing would read it, since a level rule nothing reads is a typo and
/// a player finding a `+5` that is no better than plain.
fn validate_level(d: &ItemDef, level: &Level) -> Result<(), String> {
    if d.wear.is_none() {
        return Err("a level on a thing that is never worn does nothing".into());
    }
    if level.max.is_none_or(|max| max < 1) {
        return Err("a level that reaches none: its max is the highest it is found at, 1 or more".into());
    }
    if level.min.is_some() {
        return Err("a min in the level: nothing here is found below plain".into());
    }
    // Written at all, even as 0, where nothing reads it: an explicit
    // number is a claim about one the thing has, and a reader who finds
    // it on a helmet goes looking for the helmet's blow.
    if level.armor.is_some() && d.armor == 0 {
        return Err("armor in the level, on a thing with no armor of its own".into());
    }
    if level.damage.is_some() && !attacks(d) {
        return Err("damage in the level, on a thing that does not attack".into());
    }
    if level.charges.is_some() || !level.stats.is_empty() {
        return Err("charges or stats in the level: nothing here grows either".into());
    }
    if level.dark_sight.is_some() && d.dark_sight.is_none() {
        return Err("dark_sight in the level, on a thing that lends no dark sight".into());
    }
    // A level never quickens a pulse past a turn, since a plate that pulsed
    // several times each time its wearer acts is a rule nobody wrote on
    // purpose; a period written below a turn is the thing at `+0`, and a
    // level only never takes it lower.
    let every = d.triggers.iter().find(|t| t.on == "pulse").and_then(|t| t.every);
    if let (Some(every), Some(pulse), Some(max)) = (every, level.pulse, level.max) {
        let fastest = i64::from(every) + i64::from(pulse) * i64::from(max);
        if fastest < i64::from(every.min(rl_engine::rl_core::turn::BASE_ACTION_COST)) {
            return Err(format!("pulse in the level, quickening its clock to {fastest} at +{max}, past a turn"));
        }
    }
    // `turns` reaches only an `Inflict` and `amount` only a `Harm` or a
    // `Mend`, the three engine effects that read a bonus; Foundry registers
    // no effect of its own, so the authored kind names are the whole
    // answer.
    if level.turns.is_some_and(|t| t != 0) && !lands_any(d, &["Inflict"]) {
        return Err("turns in the level, on a thing whose effects inflict no status".into());
    }
    if level.amount.is_some_and(|a| a != 0) && !lands_any(d, &["Harm", "Mend"]) {
        return Err("amount in the level, on a thing whose effects neither harm nor mend".into());
    }
    Ok(())
}

/// Whether any effect `d` holds, shared or on one of its triggers' own
/// lists, is one of `kinds`.
fn lands_any(d: &ItemDef, kinds: &[&str]) -> bool {
    let own = d.triggers.iter().filter_map(|t| t.effects.as_deref()).flatten();
    d.effects.iter().chain(own).any(|spec| kinds.contains(&spec.kind.as_str()))
}

/// The parts a thing of `d`, the definition `id`, is made of: its own, its
/// `triggers` as the armory built them, and its level with what Foundry
/// infers under it, or every reason it cannot be.
fn parts_of(d: &ItemDef, id: Id<ItemDef>, triggers: Triggers, registries: &Registries) -> Result<ItemParts, Vec<String>> {
    let wear = d.wear.as_ref().map(|w| w.shape(&registries.slots)).transpose().map_err(|e| vec![e])?;
    let parts = ItemParts {
        wear,
        tags: d.tags.iter().map(|t| t.id()).collect(),
        triggers,
        charges: d.consumable,
        armor: d.armor,
        resists: parts::resistances(&d.resists),
        attacks: Attacks { melee: d.melee, ranged: d.ranged, throw: d.throw, cost: d.cost },
        ammo: d.ammo.map(|t| t.id()),
        stack: d.stack.then_some(id.index() as u64),
        dark_sight: d.dark_sight,
        level: level_of(d),
        ..ItemParts::default()
    };
    match parts.check() {
        wrong if wrong.is_empty() => Ok(parts),
        wrong => Err(wrong),
    }
}

impl Armory {
    /// Loads `items.ron` against `registries`, validates it, builds every
    /// item's parts, its triggers against the effect `kinds` and `moments`
    /// registered for the run, with the `sounds` declared for it in scope
    /// so a grenade's noise names its sound, and builds the spawn table from
    /// `item_spawns.ron`; panics with every problem either file has, since
    /// a broken item file is a game that cannot start.
    ///
    /// The parts are built here rather than when an item is spawned, so a
    /// grenade naming an effect or a moment nobody registered is caught
    /// while the file is read rather than by a grenade that quietly does
    /// nothing in the middle of a run.
    pub fn load(registries: &Registries, kinds: &EffectKinds, moments: &Moments, sounds: &Sounds) -> Self {
        let names = registries.names().sounds(sounds);
        let defs = load_defs(registries);
        let table =
            rl_engine::rl_rules::loot::load(ITEM_SPAWNS_RON, &names.clone().with("item", &defs), &defs, |d: &ItemDef| d.tags.iter().map(|t| t.id()).collect())
                .unwrap_or_else(|e| panic!("assets/item_spawns.ron: {e}"));
        let levels = rl_engine::rl_rules::loot::load_levels(LEVELS_RON).unwrap_or_else(|e| panic!("assets/levels.ron: {e}"));
        // ANCHOR: triggers
        // Each definition's parts, its triggers built once against the
        // moments and effect kinds the run registered, and every problem in
        // the file named at once: a grenade that lands nothing is a typo.
        let mut parts = Vec::new();
        let mut errors = Vec::new();
        for (id, d) in defs.iter() {
            let built = Triggers::build(&d.triggers, &d.effects, moments, kinds, &names).and_then(|triggers| parts_of(d, id, triggers, registries));
            parts.push(Arc::new(built.unwrap_or_else(|mine| {
                errors.extend(mine.into_iter().map(|e| format!("{}: {e}", d.name)));
                ItemParts::default()
            })));
        }
        assert!(errors.is_empty(), "assets/items.ron: {}", errors.join("; "));
        // ANCHOR_END: triggers
        Self { defs, table, levels, parts }
    }

    /// The parts a thing of `def` is made of: what the engine spawns it
    /// from.
    pub fn parts(&self, def: Id<ItemDef>) -> &Arc<ItemParts> {
        &self.parts[def.index()]
    }
}

/// Every item definition in `items.ron`, checked, and nothing built from
/// them: what the monster roster reads its drops' names against, as the
/// armory reads its own.
pub fn load_defs(registries: &Registries) -> Registry<ItemDef> {
    let defs: Registry<ItemDef> = registries.names().load(ITEMS_RON).unwrap_or_else(|e| panic!("assets/items.ron: {e}"));
    defs.validate(validate_def).unwrap_or_else(|e| panic!("assets/items.ron: {e}"));
    defs
}

/// The effect kinds a Foundry app has, for loading an [`Armory`] where no
/// app is running, as `foundry --prefabs` does and a test with no world
/// does: whatever `add_engine_effects` declares, the `Ignite` and `Emit`
/// that `FirePlugin` and `GasPlugin` declare for the grenades, and the
/// `Noise` that `NoisePlugin` declares for them to be heard by, which is
/// what `main.rs` gives the real load. The throwaway `App` is there for
/// that and nothing else.
pub fn effect_kinds() -> EffectKinds {
    let mut app = App::new();
    app.add_engine_effects().add_effect::<rl_engine::rl_bevy::Ignite>().add_effect::<rl_engine::rl_bevy::Emit>().add_effect::<rl_engine::rl_bevy::Noise>();
    std::mem::take(&mut app.world_mut().resource_mut::<EffectKinds>())
}

/// The sounds Foundry declares beside the engine's, in the order
/// [`FoundryPlugin`](crate::plugin::FoundryPlugin) declares them: the
/// alarm's, then the two `items.ron` names, a grenade bursting and smoke
/// hissing out of one. One list, walked by the plugin and by [`sounds`],
/// so the ids an armory loaded with no app gives them are the app's.
pub const SOUNDS: [&str; 3] = [crate::droids::ALARM_SOUND, "blast", "hiss"];

/// The sounds a Foundry app declares, for loading an [`Armory`] where no
/// app is running, as [`effect_kinds`] is for its effects: the engine's
/// own, then [`SOUNDS`].
pub fn sounds() -> Sounds {
    let mut sounds = Sounds::default();
    for name in SOUNDS {
        sounds.declare(name);
    }
    sounds
}

/// Loads the [`Armory`] once, in `PreStartup`, before any run begins or is
/// continued, against the effects, moments and sounds the app registered.
///
/// Once rather than per use: every deck's scatter, every kill and every
/// crate used to read `items.ron` and build its triggers afresh, and the
/// save refreshed every turn needs it too.
pub fn load_armory(mut commands: Commands, registries: Res<Registries>, kinds: Res<EffectKinds>, moments: Res<Moments>, sounds: Res<Sounds>) {
    commands.insert_resource(Armory::load(&registries, &kinds, &moments, &sounds));
}

/// The armory and the registries it was read against, for the systems that
/// spawn items: one parameter rather than two, because every such system
/// wants both.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Content<'w> {
    armory: Res<'w, Armory>,
    registries: Res<'w, Registries>,
}

impl Content<'_> {
    /// The armory.
    pub fn armory(&self) -> &Armory {
        &self.armory
    }

    /// The registries, for whatever else a spawn needs them for.
    pub fn registries(&self) -> &Registries {
        &self.registries
    }
}

/// Marks an item with the definition it was made from, which is what a
/// save writes down and a continued run makes it again from.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemKind(pub Id<ItemDef>);

/// Spawns `id` as an item entity carrying every component its definition
/// implies: what it is called and drawn as, what it counts as, and, for
/// something worn, its shape, its armor, its resistances and whatever it
/// strikes or shoots with, and the `Heat` or `Ammo` it runs on. A
/// stackable item spawns as a stack of one; the caller merges or grows it
/// as it likes.
///
/// At `level`, which names it `cloak plate +2` and writes every number its
/// level says a level changes with the level applied: its armor, its
/// attack rolls, its clock, the [`EffectBonus`] its effects land with and
/// the dark sight it lends, by the engine's [`insert_parts`]. A thing that names no `level` is plain whatever `level`
/// says, and one that does is held between plain and its `max`, so a save
/// written before the file lowered a `max` never brings back a thing the
/// file no longer allows.
///
/// The level is applied here, once, rather than read by whatever uses the
/// thing: a save keeps only the level, and a continued run's `+2` is this
/// same function run again.
pub fn spawn_item_at(commands: &mut Commands, armory: &Armory, id: Id<ItemDef>, level: i32) -> Entity {
    let d = armory.defs.get(id);
    let parts = armory.parts(id);
    let level = parts.level_at(level).unwrap_or(0);
    let mut e =
        commands.spawn((ItemKind(id), Name::new(named_at(&d.name, level)), Glyph::new(d.glyph, Color::srgb(d.color.0, d.color.1, d.color.2)).on_layer(2)));
    // ANCHOR: use
    // Everything the engine plays, a stim's use and a grenade's landing,
    // what doing either costs the thing, its wear, its armor and its
    // attacks, from the parts the armory built once and shares with every
    // copy; the engine lands and spends them.
    insert_parts(&mut e, parts, level);
    // ANCHOR_END: use
    if d.mass {
        e.insert(MassNoun);
    }
    if let Some((per_shot, vent)) = d.heat {
        e.insert(Heat::new(per_shot, vent));
    }
    e.id()
}

/// Spawns `id` plain, at `+0`: what a stack is made as and what a test
/// hands the commando.
pub fn spawn_item(commands: &mut Commands, armory: &Armory, id: Id<ItemDef>) -> Entity {
    spawn_item_at(commands, armory, id, 0)
}

/// Spawns `count` of `id`: one stack of `count` for a thing that stacks,
/// and `count` of it otherwise, placed nowhere.
pub fn spawn_items(commands: &mut Commands, armory: &Armory, id: Id<ItemDef>, count: u32) -> Vec<Entity> {
    if armory.defs.get(id).stack {
        let stack = spawn_item(commands, armory, id);
        commands.entity(stack).insert(Stack { key: id.index() as u64, count: count.max(1) });
        return vec![stack];
    }
    (0..count).map(|_| spawn_item(commands, armory, id)).collect()
}

// ANCHOR: maker
/// The engine's loot is made here: what lies on a deck when it is first
/// entered, what a kill leaves, and what a crate or a locker holds. A
/// deck's band is its number, and Foundry has no streamed surface. The
/// armory makes each thing, rolling a level from `levels.ron` at the band
/// it is found at for a thing that names a `level`.
impl ItemMaker for Armory {
    type Def = ItemDef;

    fn make(&self, commands: &mut Commands, _: &Registries, def: Id<ItemDef>, count: u32, from: Provenance, rng: &mut rand::rngs::StdRng) -> Vec<Entity> {
        let d = self.defs.get(def);
        if d.stack {
            return spawn_items(commands, self, def, count);
        }
        // Each its own roll: two plates from one crate are two finds.
        (0..count)
            .map(|_| {
                let level = d.level.as_ref().map_or(0, |_| self.levels.roll(from.band, rng));
                spawn_item_at(commands, self, def, level)
            })
            .collect()
    }

    fn make_at(&self, commands: &mut Commands, _: &Registries, def: Id<ItemDef>, level: i32) -> Entity {
        spawn_item_at(commands, self, def, level)
    }

    fn id_of(&self, name: &str) -> Option<Id<ItemDef>> {
        self.defs.id(name)
    }

    fn name_of(&self, def: Id<ItemDef>) -> &str {
        self.defs.name(def)
    }

    fn table(&self) -> &LootTable<Id<ItemDef>> {
        &self.table
    }

    fn scatter(&self) -> ScatterRules {
        crate::loot::scatter_rules()
    }

    fn band(&self, area: LootArea) -> i32 {
        match area {
            LootArea::Place(map) => crate::decks::deck_of(map) as i32,
            LootArea::Region(_) => 0,
        }
    }
}
// ANCHOR_END: maker

#[cfg(test)]
mod tests {
    use bevy::ecs::world::CommandQueue;
    use rl_engine::rl_core::{Point, RunSeed};

    use super::*;

    /// Spawns two items by name, through a fresh `Armory` and a raw
    /// `CommandQueue`, the way a test reaches into the world without a
    /// system of its own.
    fn spawn_two(app: &mut App, a: &str, b: &str) -> (Entity, Entity) {
        let armory = crate::testing::armory_of(app);
        let mut queue = CommandQueue::default();
        let mut commands = Commands::new(&mut queue, app.world_mut());
        let ea = spawn_item(&mut commands, &armory, armory.defs.expect(a));
        let eb = spawn_item(&mut commands, &armory, armory.defs.expect(b));
        queue.apply(app.world_mut());
        (ea, eb)
    }

    /// Spawns `name`, puts it in the player's bag, and equips it through
    /// the engine's own `Equip` intent, so wearing it fires the same
    /// `ItemEvent` the real game reacts to.
    fn wear(app: &mut App, name: &str) -> (Entity, Entity) {
        app.update();
        app.update();
        let armory = crate::testing::armory_of(app);
        let id = armory.defs.expect(name);
        let mut queue = CommandQueue::default();
        let mut commands = Commands::new(&mut queue, app.world_mut());
        let item = spawn_item(&mut commands, &armory, id);
        queue.apply(app.world_mut());
        let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
        app.world_mut().get_mut::<Inventory>(player).unwrap().items.push(item);
        app.world_mut().write_message(Intent::new(player, Equip(item)));
        app.update();
        (player, item)
    }

    /// Takes `item` off `player` through the engine's own `Unequip` intent.
    fn unwear(app: &mut App, player: Entity, item: Entity) {
        app.world_mut().write_message(Intent::new(player, Unequip(item)));
        app.update();
    }

    #[test]
    fn every_item_loads_and_every_spawn_band_on_the_first_three_decks_has_something() {
        let r = crate::content::registries();
        let armory = crate::testing::armory(&r);
        assert_eq!(armory.defs.len(), 24, "sixteen things to carry, a slug, a keycard, a stim, a medkit and four grenades");
        assert!(armory.table.gaps(1..=3).is_empty(), "a deck with nothing to find");
    }

    #[test]
    fn a_weapons_file_numbers_reach_its_attack_and_a_two_hander_claims_the_off_hand() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (axe, carbine) = spawn_two(&mut app, "mono-axe", "blaster carbine");
        let world = app.world();
        let melee = world.get::<MeleeAttack>(axe).expect("the axe swings");
        assert_eq!(melee.cost, Some(140));
        let ranged = world.get::<RangedAttack>(carbine).expect("the carbine fires");
        assert_eq!((ranged.range, ranged.cost), (12, Some(100)));
        let shape = &world.get::<Wearable>(axe).unwrap().0;
        assert_eq!(shape.slots().count(), 2, "main hand and off hand");
    }

    /// The slug rifle is the pistol's economy at a rifle's reach: a slug a
    /// shot and dry on an empty bag, but a line past what the lamp shows,
    /// so it kills what the commando cannot yet see.
    #[test]
    fn a_slug_rifle_spends_a_slug_a_shot_and_reaches_past_the_lamp() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (player, rifle) = crate::testing::slug_gun_with(&mut app, "slug rifle", 2);
        let reach = app.world().get::<RangedAttack>(rifle).expect("loaded, so it has a shot").range;
        assert!(reach > crate::light::SHOULDER_LAMP.radius, "{reach} tiles, no further than the lamp");
        let struck = crate::testing::fire_at_a_target(&mut app, player, 3);
        assert_eq!(struck.len(), 2, "two slugs, two shots; the third finds nothing to fire");
        assert!(app.world().get::<RangedAttack>(rifle).is_none(), "dry");
    }

    /// The heavy repeater runs hot the fast way: its whole burst goes out
    /// in half the time a carbine's does, and then it is locked like any
    /// other energy weapon.
    #[test]
    fn a_heavy_repeater_gets_its_burst_off_in_half_the_time_a_carbine_takes() {
        let burst = |name: &str| {
            let mut app = crate::testing::headless(RunSeed(1));
            crate::testing::settle(&mut app);
            let player = crate::testing::empty_handed(&mut app);
            let gun = crate::testing::equip_new(&mut app, player, name);
            let before = crate::testing::clock(&app);
            let mut shots = 0;
            while app.world().get::<RangedAttack>(gun).is_some() {
                assert!(shots < 10, "{name} never locked");
                shots += crate::testing::fire_at_a_target(&mut app, player, 1).len();
            }
            (shots, crate::testing::clock(&app) - before)
        };
        let (repeater_shots, repeater_time) = burst("heavy repeater");
        let (carbine_shots, carbine_time) = burst("blaster carbine");
        assert_eq!(repeater_shots, carbine_shots, "the same five shots before it locks");
        assert!(repeater_time * 2 <= carbine_time, "{repeater_time} against the carbine's {carbine_time}");
    }

    /// What a suit resists is resisted by whoever wears it: twenty points
    /// of energy on a commando in composite plate meet flesh's quarter and
    /// the plate's tenth together, seven off, and then the plate's two
    /// points of armor, so eleven land. Nothing is copied onto the wearer
    /// to make that so, and the tenth leaves with the plate.
    #[test]
    fn composite_plate_resists_its_tenth_of_a_bolt_on_top_of_what_flesh_resists() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (player, plate) = wear(&mut app, "composite plate");
        let energy = app.world().resource::<Registries>().damage_kinds.expect("energy");
        let bolt = |app: &mut App| {
            app.world_mut().entity_mut(player).insert(Health::full(100));
            app.world_mut().write_message(DamageEvent::new(player, rl_engine::rl_rules::Hit::from_source(None, energy, 20)));
            app.update();
            100 - health(app, player)
        };
        assert_eq!(bolt(&mut app), 20 - 7 - 2, "a quarter and a tenth resisted, then two points of plate");
        unwear(&mut app, player, plate);
        assert_eq!(bolt(&mut app), 20 - 5, "and taken off, flesh's quarter alone");
    }

    #[test]
    fn wearing_the_rangefinder_gives_dark_sight_and_taking_it_off_takes_it_away() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (player, helmet) = wear(&mut app, "rangefinder helmet");
        assert_eq!(app.world().get::<DarkSight>(player).map(|d| d.0), Some(6));
        unwear(&mut app, player, helmet);
        assert_eq!(app.world().get::<DarkSight>(player), None);
    }

    /// The reviewer's own reproduction, Fix round 1, finding 1: a jam that
    /// stored the radius it took away and a `grant_dark_sight` that wrote
    /// `DarkSight` from gear alone fought over the same component the
    /// moment a jammed wearer took the helmet off, and the wearer kept
    /// radar for good with no helmet on. `DarkSight` derived fresh every
    /// pass by the engine's fold has nothing left to fight over: with the
    /// helmet gone, there is nothing for it to read once the jam clears.
    #[test]
    fn a_jammed_wearer_who_takes_the_helmet_off_has_no_dark_sight_once_the_jam_clears() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (player, helmet) = wear(&mut app, "rangefinder helmet");
        assert_eq!(app.world().get::<DarkSight>(player).map(|d| d.0), Some(6));
        crate::testing::hit(&mut app, player, "ion", 1);
        assert_eq!(app.world().get::<DarkSight>(player), None, "jammed");
        unwear(&mut app, player, helmet);
        crate::testing::pass_turns(&mut app, 3);
        assert_eq!(app.world().get::<DarkSight>(player), None, "no helmet, so nothing to see by once the jam clears either");
    }

    #[test]
    fn a_jammed_wearer_who_swaps_helmets_gets_the_new_ones_radius_once_the_jam_clears() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (player, old_helmet) = wear(&mut app, "rangefinder helmet");
        crate::testing::hit(&mut app, player, "ion", 1);
        assert_eq!(app.world().get::<DarkSight>(player), None, "jammed");
        unwear(&mut app, player, old_helmet);
        // A second sensor helmet, spawned by hand at a radius nothing in
        // the roster names, so a leftover 6 from the old one could never
        // be mistaken for the new one's own 9.
        let head = app.world().resource::<Registries>().slots.expect("head");
        let mut queue = CommandQueue::default();
        let mut commands = Commands::new(&mut queue, app.world_mut());
        let new_helmet = commands.spawn((Item, Wearable(rl_engine::rl_rules::EquipShape::in_slot(head)), WornDarkSight(9))).id();
        queue.apply(app.world_mut());
        app.world_mut().get_mut::<Inventory>(player).unwrap().items.push(new_helmet);
        app.world_mut().write_message(Intent::new(player, Equip(new_helmet)));
        app.update();
        crate::testing::pass_turns(&mut app, 3);
        assert_eq!(app.world().get::<DarkSight>(player).map(|d| d.0), Some(9), "the new helmet's own radius, not the old one's");
    }

    /// A definition with fields left at their most inert: no slot, no
    /// attack, no economy, nothing to spawn with. Tests that care about one
    /// property override just that field, rather than restating all
    /// twenty-three every time.
    fn blank_def(name: &str) -> ItemDef {
        ItemDef {
            name: name.to_string(),
            mass: false,
            glyph: '?',
            color: (0.0, 0.0, 0.0),
            wear: None,
            tags: Vec::new(),
            effects: Vec::new(),
            triggers: Vec::new(),
            consumable: None,
            armor: 0,
            resists: Vec::new(),
            melee: None,
            ranged: None,
            throw: None,
            cost: None,
            heat: None,
            ammo: None,
            dark_sight: None,
            level: None,
            stack: false,
        }
    }

    /// A level reaching `max` and saying nothing else, so a level adds
    /// only what is inferred.
    fn level_to(max: i32) -> Level {
        Level { max: Some(max), ..Level::default() }
    }

    /// The commando, wounded by `hurt`, alone on a quiet deck one: the
    /// droids and the rats are taken off it, because the only thing that
    /// may move this player's health over the turns a test waits is what
    /// the test gave them.
    fn alone_and_wounded(seed: u64, hurt: i32) -> (App, Entity) {
        let mut app = crate::testing::headless(RunSeed(seed));
        crate::testing::arrive_on(&mut app, 1);
        let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
        let others: Vec<Entity> = {
            let world = app.world_mut();
            let mut q = world.query_filtered::<Entity, (With<Actor>, Without<Player>)>();
            q.iter(world).collect()
        };
        for other in others {
            app.world_mut().entity_mut(other).despawn();
        }
        // Put at the wound rather than dealt one: `testing::hit` writes the
        // report a blow ends in, which reacts but never takes a point off,
        // and a real blow would drag armor, resistances and a profile in
        // between the test and the thing it is measuring.
        {
            let mut health = app.world_mut().get_mut::<Health>(player).expect("the commando has health");
            health.current = health.max - hurt;
        }
        (app, player)
    }

    /// Puts `count` of `name` in the player's bag and returns the stack.
    fn carry(app: &mut App, player: Entity, name: &str, count: u32) -> Entity {
        let armory = crate::testing::armory_of(app);
        let id = armory.defs.expect(name);
        let item = {
            let mut queue = CommandQueue::default();
            let mut commands = Commands::new(&mut queue, app.world_mut());
            let item = spawn_item(&mut commands, &armory, id);
            commands.entity(item).insert(Stack { key: id.index() as u64, count });
            queue.apply(app.world_mut());
            item
        };
        app.world_mut().entity_mut(player).insert(Inventory { items: vec![item] });
        app.update();
        item
    }

    /// What the commando has left.
    fn health(app: &App, who: Entity) -> i32 {
        app.world().get::<Health>(who).expect("the commando has health").current
    }

    /// A stim closes a wound on the spot and the shot is gone with it.
    ///
    /// Nothing of Foundry's own runs here: the item's `use` trigger lands
    /// the mend where the commando stands, and its one charge takes one off
    /// the stack. The roll is `2d4+5`, so the
    /// gain is between seven and thirteen whatever the dice say, which is
    /// what makes ten its midpoint and the medkit's twenty twice it.
    #[test]
    fn a_stim_closes_a_wound_at_once_and_the_shot_is_spent() {
        let (mut app, player) = alone_and_wounded(3, 20);
        let stim = carry(&mut app, player, "stim", 2);
        let before = health(&app, player);

        app.world_mut().write_message(Intent::new(player, UseItem(stim)));
        crate::testing::settle(&mut app);

        let gained = health(&app, player) - before;
        assert!((7..=13).contains(&gained), "2d4+5 closed at once, not {gained}");
        assert_eq!(app.world().get::<Stack>(stim).map(|s| s.count), Some(1), "one shot off the stack, spent using it");
    }

    /// A thing in the bag is not something the commando knows how to do.
    ///
    /// The stim and the medkit were abilities for a day, granted by the item
    /// that carried them, and the abilities screen listed both beside the
    /// one thing the commando had actually been taught. What a used thing
    /// does is on the thing now, so `Known` is what the run has learned and
    /// nothing else.
    #[test]
    fn carrying_the_medical_pair_teaches_the_commando_nothing() {
        let (mut app, player) = alone_and_wounded(6, 10);
        carry(&mut app, player, "stim", 1);
        app.update();
        assert!(app.world().get::<Known>(player).is_none_or(|k| k.iter().count() == 0), "a stim in the bag is not an ability");
        assert!(!crate::testing::knows(&app, player, "stims"), "and the upgrade's own is still unlearned");
    }

    /// A medkit is worth twice a stim and pays it two a turn over ten,
    /// which is ten turns of a deck the commando has to live through.
    ///
    /// Ten turns exactly: the tenth mends and the eleventh does not, so
    /// the gain is `MEND_PER_TURN * 10` and no more. The status is the
    /// engine's own and mends rather than ticks damage of some kind, so
    /// plate and resistances have nothing to say about it.
    #[test]
    fn a_medkit_is_worth_twice_a_stim_spread_over_ten_turns() {
        let (mut app, player) = alone_and_wounded(4, 25);
        let medkit = carry(&mut app, player, "medkit", 1);
        let before = health(&app, player);
        let mending = app.world().resource::<Registries>().statuses.expect("mending");

        app.world_mut().write_message(Intent::new(player, UseItem(medkit)));
        crate::testing::settle(&mut app);
        assert!(app.world().get::<Afflicted>(player).is_some_and(|a| a.0.has(mending)), "the gel is working");
        assert_eq!(health(&app, player) - before, crate::content::MEND_PER_TURN, "the first of the ten turns is the turn it is used");

        crate::testing::pass_turns(&mut app, 9);
        let gained = health(&app, player) - before;
        assert_eq!(gained, crate::content::MEND_PER_TURN * 10, "two a turn for ten turns, twice a stim's ten");
        assert!(!app.world().get::<Afflicted>(player).is_some_and(|a| a.0.has(mending)), "and it is done");

        crate::testing::pass_turns(&mut app, 3);
        assert_eq!(health(&app, player) - before, gained, "nothing after the ten");
        assert!(app.world().get_entity(medkit).is_err(), "the last kit off the stack goes with it");
    }

    /// A second medkit starts the ten turns again rather than mending
    /// twice as fast: `mending` refreshes, which is the engine's default
    /// and the rule that keeps a pack of kits a longer recovery instead of
    /// a bigger one.
    #[test]
    fn a_second_medkit_starts_the_ten_turns_again_rather_than_doubling_the_rate() {
        let (mut app, player) = alone_and_wounded(5, 25);
        let kits = carry(&mut app, player, "medkit", 2);
        let before = health(&app, player);

        app.world_mut().write_message(Intent::new(player, UseItem(kits)));
        crate::testing::settle(&mut app);
        crate::testing::pass_turns(&mut app, 2);
        // The turn it went on and the two after it: three turns of gel.
        let one_kit = health(&app, player) - before;
        assert_eq!(one_kit, crate::content::MEND_PER_TURN * 3, "two a turn from the turn it went on");

        app.world_mut().write_message(Intent::new(player, UseItem(kits)));
        crate::testing::settle(&mut app);
        crate::testing::pass_turns(&mut app, 2);
        assert_eq!(health(&app, player) - before, one_kit + crate::content::MEND_PER_TURN * 3, "still two a turn with the second kit in, not four");
    }

    /// Throws one of the grenades `player` carries at `aim`, through the
    /// engine's own throw action, the way the pack's throw key does.
    fn throw_grenade(app: &mut App, player: Entity, grenade: Entity, aim: Point) {
        app.world_mut().write_message(Intent::new(player, Throw { item: grenade, at: aim }));
        crate::testing::settle(app);
    }

    /// Every item lying on `at`.
    fn lying_at(app: &mut App, at: Point) -> Vec<Entity> {
        let world = app.world_mut();
        let mut q = world.query_filtered::<(Entity, &Position), With<Item>>();
        q.iter(world).filter(|(_, p)| p.0 == at).map(|(e, _)| e).collect()
    }

    fn at(app: &App, e: Entity) -> Point {
        app.world().get::<Position>(e).expect("it stands somewhere").0
    }

    /// A frag grenade is thrown and lands like a blast: the droid it is
    /// thrown at is hurt, the grenade that did it is one fewer in the bag,
    /// and nothing is left lying where it burst.
    #[test]
    fn a_frag_grenade_bursts_on_the_droid_it_is_thrown_at_and_is_spent_doing_it() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (droid, player) = crate::testing::droid_down_a_lane(&mut app, "line droid", 4, 4);
        let frags = carry(&mut app, player, "frag grenade", 2);
        let before = health(&app, droid);
        let aim = at(&app, droid);
        throw_grenade(&mut app, player, frags, aim);
        assert!(app.world().get::<Health>(droid).is_none_or(|h| h.current < before), "the blast reached it");
        assert_eq!(app.world().get::<Stack>(frags).map(|s| s.count), Some(1), "one grenade off the stack");
        assert!(lying_at(&mut app, aim).is_empty(), "and the one thrown is spent, not lying there");
    }

    /// An incendiary is a fire the commando starts: the deck plate it lands
    /// on has nothing to burn, and burns anyway for the turns it names.
    #[test]
    fn an_incendiary_leaves_the_deck_it_lands_on_burning() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (droid, player) = crate::testing::droid_down_a_lane(&mut app, "line droid", 4, 4);
        let grenade = carry(&mut app, player, "incendiary grenade", 1);
        let aim = at(&app, droid);
        throw_grenade(&mut app, player, grenade, aim);
        assert!(app.world().resource::<Fire>().is_burning(aim), "alight where it landed");
        crate::testing::pass_turns(&mut app, 1);
        assert!(app.world().resource::<Fire>().is_burning(aim), "and still burning a turn later");
    }

    /// An ion grenade blinds every radar in the burst at once, not only the
    /// one it was thrown at.
    #[test]
    fn an_ion_grenade_jams_every_radar_in_its_burst() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (first, player) = crate::testing::droid_down_a_lane(&mut app, "probe droid", 4, 4);
        let beside = at(&app, first).offset(0, 1);
        let floor = app.world().resource::<WorldMap>().tile(at(&app, first)).expect("the lane is loaded");
        app.world_mut().resource_mut::<WorldMap>().set_tile(beside, floor);
        let registries = app.world().resource::<Registries>().clone();
        let roster = crate::droids::Roster::load(&registries);
        let map = app.world().resource::<WorldMap>().current();
        let mut queue = CommandQueue::default();
        let mut commands = Commands::new(&mut queue, app.world_mut());
        let second = crate::droids::spawn_monster(&mut commands, &roster, roster.defs.expect("probe droid"), beside, map, &registries);
        queue.apply(app.world_mut());
        let grenade = carry(&mut app, player, "ion grenade", 1);
        for probe in [first, second] {
            assert!(app.world().get::<DarkSight>(probe).is_some(), "radar up before the throw");
        }
        let aim = at(&app, first);
        throw_grenade(&mut app, player, grenade, aim);
        for probe in [first, second] {
            assert_eq!(app.world().get::<DarkSight>(probe), None, "{probe:?} jammed");
        }
    }

    /// A smoke grenade is a burst of smoke big enough to fight in, and does
    /// no harm. On open deck it hides a room's worth of cells the moment it
    /// lands, still hides some of it nine turns on, and then thins away.
    #[test]
    fn a_smoke_grenade_hides_a_room_s_worth_of_deck_for_ten_turns_then_thins() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (droid, player) = crate::testing::droid_down_a_lane(&mut app, "line droid", 5, 5);
        crate::testing::clear_droids(&mut app, &[droid]);
        let (aim, before) = (at(&app, droid), health(&app, droid));
        let floor = app.world().resource::<WorldMap>().tile(aim).expect("the lane is loaded");
        {
            let mut map = app.world_mut().resource_mut::<WorldMap>();
            for dy in -9..=9 {
                for dx in -9..=9 {
                    map.set_tile(aim.offset(dx, dy), floor);
                }
            }
        }
        let grenade = carry(&mut app, player, "smoke grenade", 1);
        throw_grenade(&mut app, player, grenade, aim);
        let registries = app.world().resource::<Registries>();
        let smoke = registries.gases.expect("smoke");
        let veils_at = registries.gases.get(smoke).veils_at.expect("smoke hides what is behind it");
        let hidden = |app: &App| app.world().resource::<Gases>().cells(smoke).filter(|(_, c)| *c >= veils_at).count();
        let landed = hidden(&app);
        assert!(landed >= 25, "a room's worth hidden where it landed, not {landed} cells");
        assert_eq!(health(&app, droid), before, "and nobody hurt by it");
        // Out of the way, so nine turns of waiting in the open are nine
        // turns nobody is shot in.
        app.world_mut().despawn(droid);
        crate::testing::pass_turns(&mut app, 9);
        assert!(hidden(&app) > 0, "still hiding some of the deck nine turns on");
        crate::testing::pass_turns(&mut app, 40);
        assert_eq!(app.world().resource::<Gases>().cells(smoke).count(), 0, "and thinned away to nothing");
    }

    /// A stim is used, never thrown: a use trigger, and no throw in the file,
    /// so the pack offers the one and not the other.
    #[test]
    fn a_stim_has_a_use_trigger_and_no_throw() {
        let r = crate::content::registries();
        let armory = crate::testing::armory(&r);
        let stim = armory.defs.get(armory.defs.expect("stim"));
        assert!(stim.throw.is_none());
        assert_eq!(stim.triggers.iter().map(|t| t.on.as_str()).collect::<Vec<_>>(), vec!["use"]);
    }

    /// The two plates read as the file writes them: the nanite plate's
    /// clock is its pulse trigger's, and what a level does to it is its
    /// level block's; the cloak plate's charge refills only while worn,
    /// and each level adds two turns to what it inflicts.
    #[test]
    fn the_plates_load_with_their_clock_on_the_trigger_and_their_levels_in_the_level_block() {
        let r = crate::content::registries();
        let armory = crate::testing::armory(&r);
        let nanite = armory.defs.get(armory.defs.expect("nanite plate"));
        assert_eq!(nanite.triggers.iter().find(|t| t.on == "pulse").and_then(|t| t.every), Some(1000));
        assert_eq!(nanite.level.as_ref().map(|l| (l.max, l.armor, l.pulse)), Some((Some(9), Some(0), Some(-100))));
        let cloak = armory.defs.get(armory.defs.expect("cloak plate"));
        assert_eq!(cloak.consumable.map(|c| (c.recharge, c.while_worn)), Some((Some(4000), true)));
        assert_eq!(cloak.level.as_ref().map(|l| (l.max, l.armor, l.turns)), Some((Some(9), Some(0), Some(2))));
    }

    /// The parts of `d` as the armory builds them, or every reason the
    /// engine or Foundry gives that they cannot be.
    fn built(d: &ItemDef, r: &Registries) -> Result<ItemParts, Vec<String>> {
        let triggers = Triggers::build(&d.triggers, &d.effects, &Moments::default(), &effect_kinds(), &r.names().sounds(&sounds()))?;
        parts_of(d, Id::from_raw(0), triggers, r)
    }

    /// Each of Foundry's rules for a level refuses its own case with its
    /// own message, and so does each of the engine's for a stack, a charge
    /// and a clock that nothing would ever play: a level rule nothing
    /// reads is a typo in the file rather than a plate.
    #[test]
    fn each_misplaced_level_rule_fails_to_validate_with_its_own_message() {
        let r = crate::content::registries();
        let armory = crate::testing::armory(&r);
        let worn = |name: &str| ItemDef { wear: Some(parts::Wear { slot: Id::from_raw(0).into(), either: false, also: Vec::new() }), ..blank_def(name) };
        let nanite = armory.defs.get(armory.defs.expect("nanite plate"));
        let cloak = armory.defs.get(armory.defs.expect("cloak plate"));
        let cases: Vec<(ItemDef, &str)> = vec![
            (ItemDef { level: Some(level_to(2)), ..blank_def("loose level") }, "never worn"),
            (ItemDef { level: Some(level_to(0)), ..worn("plain for good") }, "reaches none"),
            (ItemDef { level: Some(Level { min: Some(-1), ..level_to(3) }), ..worn("a curse") }, "below plain"),
            (ItemDef { level: Some(Level { turns: Some(2), ..level_to(9) }), ..worn("nothing to lengthen") }, "inflict no status"),
            (
                ItemDef { level: Some(Level { turns: Some(2), ..level_to(9) }), triggers: nanite.triggers.clone(), ..worn("a mend that lasts") },
                "inflict no status",
            ),
            (ItemDef { level: Some(Level { amount: Some(1), ..level_to(9) }), ..worn("nothing to strengthen") }, "neither harm nor mend"),
            (
                ItemDef {
                    level: Some(Level { amount: Some(1), ..level_to(9) }),
                    consumable: cloak.consumable,
                    triggers: cloak.triggers.clone(),
                    ..worn("a stronger cloak")
                },
                "neither harm nor mend",
            ),
            (ItemDef { level: Some(Level { damage: Some(2), ..level_to(5) }), ..worn("harmless edge") }, "does not attack"),
            (ItemDef { level: Some(Level { damage: Some(0), ..level_to(5) }), ..worn("harmless and says so") }, "does not attack"),
            (ItemDef { level: Some(Level { armor: Some(1), ..level_to(3) }), ..worn("paper shirt") }, "no armor of its own"),
            (ItemDef { level: Some(Level { armor: Some(0), ..level_to(3) }), ..worn("paper shirt that says so") }, "no armor of its own"),
            (ItemDef { level: Some(Level { pulse: Some(-200), ..level_to(9) }), triggers: nanite.triggers.clone(), ..worn("a racing clock") }, "past a turn"),
            (ItemDef { dark_sight: Some(6), ..blank_def("loose sensor") }, "never worn"),
        ];
        for (d, says) in cases {
            let err = validate_def(&d, &armory.defs).expect_err(&d.name);
            assert!(err.contains(says), "{}: {err:?} does not say {says:?}", d.name);
        }
        let engine: Vec<(ItemDef, &str)> = vec![
            (ItemDef { level: Some(level_to(2)), stack: true, ..worn("a stack of fine things") }, "stacks takes no level"),
            (ItemDef { consumable: cloak.consumable, ..blank_def("loose charge") }, "refills only while worn"),
            (ItemDef { triggers: nanite.triggers.clone(), ..blank_def("loose clock") }, "never comes round"),
            (ItemDef { level: Some(Level { pulse: Some(-100), ..level_to(9) }), ..worn("idle clock") }, "no pulse trigger"),
        ];
        for (d, says) in engine {
            let err = built(&d, &r).map(|_| ()).expect_err(&d.name);
            assert!(err.iter().any(|e| e.contains(says)), "{}: {err:?} does not say {says:?}", d.name);
        }
        let d = ItemDef { level: Some(level_to(3)), ..worn("a fine plate") };
        assert!(validate_def(&d, &armory.defs).is_ok() && built(&d, &r).is_ok(), "and a worn level that reaches a level is fine");
        let d = ItemDef {
            level: Some(Level { turns: Some(2), ..level_to(9) }),
            consumable: cloak.consumable,
            triggers: cloak.triggers.clone(),
            ..worn("a longer cloak")
        };
        assert!(validate_def(&d, &armory.defs).is_ok(), "turns on a thing that inflicts a status is fine");
        let d = ItemDef { level: Some(Level { amount: Some(1), ..level_to(9) }), triggers: nanite.triggers.clone(), ..worn("a stronger mend") };
        assert!(validate_def(&d, &armory.defs).is_ok(), "and amount on a thing that mends is fine");
    }

    /// An item written the old way, a clock or an attunement at the top of
    /// the item or an `enchant` block, fails to load rather than loading
    /// plain and quietly doing less; and so does dark sight written the
    /// old way, `(tiles:, per_level:)`, since a level's tiles are the
    /// level's `dark_sight` now.
    #[test]
    fn an_item_with_its_level_rules_in_the_old_places_fails_to_load() {
        let r = crate::content::registries();
        for (field, item) in [
            ("pulse", r#"[(name: "x", glyph: '[', color: (0.0, 0.0, 0.0), wear: (slot: "torso"), pulse: (every: 1000, per_level: -100, fastest: 100))]"#),
            ("attuned", r#"[(name: "x", glyph: '[', color: (0.0, 0.0, 0.0), wear: (slot: "torso"), attuned: true)]"#),
            ("enchant", r#"[(name: "x", glyph: '[', color: (0.0, 0.0, 0.0), wear: (slot: "torso"), enchant: (max: 9))]"#),
            ("integer", r#"[(name: "x", glyph: '[', color: (0.0, 0.0, 0.0), wear: (slot: "torso"), dark_sight: (tiles: 6, per_level: 1))]"#),
        ] {
            let err = r.names().load::<ItemDef>(item).map(|_| ()).expect_err(field);
            assert!(err.to_string().contains(field), "{err} does not say {field:?}");
        }
    }

    /// The nanite plate knits a point back every ten turns worn, counted
    /// from when it went on.
    #[test]
    fn the_nanite_plate_mends_a_point_every_ten_turns_worn() {
        let (mut app, player) = alone_and_wounded(11, 10);
        let before = health(&app, player);
        crate::testing::equip_new(&mut app, player, "nanite plate");
        crate::testing::pass_turns(&mut app, 8);
        assert_eq!(health(&app, player), before, "not yet");
        crate::testing::pass_turns(&mut app, 2);
        assert_eq!(health(&app, player), before + 1, "a point in ten turns");
    }

    /// Every `Healed` on anyone, recorded as it is written: a headless
    /// app rotates its message buffers on wall time, so draining the queue
    /// afterwards can find it already empty and prove nothing.
    #[derive(Resource, Default)]
    struct Mended(Vec<(Entity, i32)>);

    fn record_mended(mut healed: MessageReader<Healed>, mut out: ResMut<Mended>) {
        out.0.extend(healed.read().map(|h| (h.target, h.restored)));
    }

    /// Worn at full health it mends nothing and says nothing: a whole
    /// commando's log is not a list of mends.
    #[test]
    fn the_nanite_plate_on_a_whole_commando_says_nothing() {
        let (mut app, player) = alone_and_wounded(12, 0);
        app.init_resource::<Mended>().add_systems(PostUpdate, record_mended);
        crate::testing::equip_new(&mut app, player, "nanite plate");
        crate::testing::pass_turns(&mut app, 21);
        let mine: Vec<i32> = app.world().resource::<Mended>().0.iter().filter(|(t, _)| *t == player).map(|(_, d)| *d).collect();
        assert_eq!(mine.len(), 2, "two pulses came round in twenty-one turns: {mine:?}");
        assert!(mine.iter().all(|d| *d == 0), "and each mended nothing, so the narrator says nothing: {mine:?}");
    }

    /// A `+2` nanite plate comes round every eight turns, a `+9` one every
    /// turn, and a `+12` one, held at its `max`, no faster.
    #[test]
    fn a_nanite_plate_quickens_with_its_level_to_every_turn_at_plus_nine() {
        let r = crate::content::registries();
        let armory = crate::testing::armory(&r);
        let every = |level| leveled(&armory, "nanite plate", level).pulse;
        assert_eq!((every(0), every(2), every(9), every(12)), (Some(1000), Some(800), Some(100), Some(100)));
        assert_eq!(leveled(&armory, "nanite plate", 9).armor, Some(1), "and its armor stays as written, its level saying armor: 0");
    }

    /// A level that would quicken a pulse past a turn by its `max` is
    /// refused when the file is read, while one that stops at a turn is
    /// not, and a period written below a turn may not be quickened at all,
    /// since it is what the thing does at `+0`; slowing one still may.
    #[test]
    fn a_level_never_quickens_a_pulse_past_a_turn_and_never_moves_a_period_written_below_one() {
        let r = crate::content::registries();
        let armory = crate::testing::armory(&r);
        let nanite = armory.defs.get(armory.defs.expect("nanite plate"));
        let clock = |every: u32, pulse: i32, max: i32| {
            let mut triggers = nanite.triggers.clone();
            triggers[0].every = Some(every);
            let d = ItemDef { triggers, level: Some(Level { pulse: Some(pulse), ..level_to(max) }), ..nanite.clone() };
            validate_def(&d, &armory.defs).is_ok()
        };
        assert!(clock(1000, -100, 9), "a turn at +9");
        assert!(!clock(1000, -100, 10), "past a turn at +10");
        assert!(!clock(50, -100, 1), "a half-turn clock is not quickened");
        assert!(clock(50, 100, 2), "and a level that slows it still does");
    }

    /// Every attacker's swing, recorded as it is written: a miss spends no
    /// health, and a test that read health would take a miss for a droid
    /// that never swung.
    #[derive(Resource, Default)]
    struct Swung(Vec<Entity>);

    fn record_swings(mut struck: MessageReader<Struck>, mut swung: ResMut<Swung>) {
        swung.0.extend(struck.read().map(|s| s.attacker));
    }

    /// The cloak plate is empty when it goes on and charged forty turns
    /// later; used then, a droid alert to the commando one step away never
    /// swings at them while it lasts.
    #[test]
    fn the_cloak_plate_hides_the_commando_from_an_adjacent_droid_once_it_has_charged() {
        let (mut app, player) = alone_and_wounded(15, 0);
        let plate = crate::testing::equip_new(&mut app, player, "cloak plate");
        assert_eq!(app.world().get::<Consumable>(plate).map(|c| c.left), Some(0), "empty when it went on");
        crate::testing::pass_turns(&mut app, 40);
        assert_eq!(app.world().get::<Consumable>(plate).map(|c| c.left), Some(1), "charged after forty turns worn");
        // Only now the droid, so forty turns of waiting are not forty turns
        // of being hit.
        let (droid, _) = crate::testing::droid_facing_player(&mut app, "line droid", 1);
        crate::testing::alert(&mut app, droid, player);
        app.init_resource::<Swung>().add_systems(PostUpdate, record_swings);
        app.world_mut().write_message(Intent::new(player, UseItem(plate)));
        crate::testing::settle(&mut app);
        assert!(app.world().get::<Unseen>(player).is_some(), "cloaked");
        crate::testing::pass_turns(&mut app, 3);
        assert!(!app.world().resource::<Swung>().0.contains(&droid), "never swung at while cloaked");
    }

    /// How many turns the commando stays unseen after using a charged
    /// cloak plate at `level`, alone on a quiet deck so nothing ends it
    /// early, counted in waits until `Unseen` is gone.
    fn turns_cloaked_by_a_plate_at(level: i32) -> u32 {
        let (mut app, player) = alone_and_wounded(13, 0);
        let armory = crate::testing::armory_of(&app);
        let plate = {
            let mut queue = CommandQueue::default();
            let mut commands = Commands::new(&mut queue, app.world_mut());
            let plate = spawn_item_at(&mut commands, &armory, armory.defs.expect("cloak plate"), level);
            queue.apply(app.world_mut());
            plate
        };
        let named = if level == 0 { "cloak plate".to_string() } else { format!("cloak plate +{level}") };
        assert_eq!(app.world().get::<Name>(plate).map(|n| n.as_str().to_string()), Some(named));
        app.world_mut().get_mut::<Inventory>(player).unwrap().items.push(plate);
        app.world_mut().write_message(Intent::new(player, Equip(plate)));
        app.update();
        app.world_mut().get_mut::<Consumable>(plate).unwrap().left = 1;
        app.world_mut().write_message(Intent::new(player, UseItem(plate)));
        crate::testing::settle(&mut app);
        assert!(app.world().get::<Unseen>(player).is_some(), "unseen once the plate is used");
        let mut turns = 0;
        while app.world().get::<Unseen>(player).is_some() {
            assert!(turns < 60, "a +{level} cloak that never wore off");
            crate::testing::pass_turns(&mut app, 1);
            turns += 1;
        }
        turns
    }

    /// Each level of a cloak plate hides the commando two turns longer: a
    /// `+2` lasts four turns past a plain one's ten.
    ///
    /// Counted as waits after the use settles, the plain plate's ten come
    /// to nine: the status ticks once in the pass it lands, which is the
    /// use's own turn, so the tenth turn is the one it was used on.
    #[test]
    fn a_cloak_plates_level_adds_two_turns_each() {
        let plain = turns_cloaked_by_a_plate_at(0);
        let plus_two = turns_cloaked_by_a_plate_at(2);
        assert_eq!(plain, 9, "a plain cloak's ten turns, the first of them the use's own");
        assert_eq!(plus_two, plain + 4, "two more turns for each of two levels, fourteen in all");
    }

    /// The commando's `@` fades the pass the cloak goes on and is white
    /// again the pass it wears off.
    #[test]
    fn the_commando_is_drawn_faded_while_unseen() {
        let (mut app, player) = alone_and_wounded(14, 0);
        let fg = |app: &App| app.world().get::<Glyph>(player).map(|g| g.fg);
        assert_eq!(fg(&app), Some(crate::run::COMMANDO));
        let cloaked = app.world().resource::<Registries>().statuses.expect("cloaked");
        app.world_mut().write_message(Afflict { target: player, status: cloaked, turns: 2, by: None, held_by: None });
        crate::testing::pass_turns(&mut app, 1);
        assert_eq!(fg(&app), Some(crate::run::COMMANDO_UNSEEN), "faded while unseen");
        crate::testing::pass_turns(&mut app, 3);
        assert_eq!(fg(&app), Some(crate::run::COMMANDO), "and white once it wore off");
    }

    /// The cloak lasts only while the plate is worn: used charged and then
    /// taken off, the commando is seen again and drawn white in the pass it
    /// comes off, and the log says so once, and never again when the ten
    /// turns it would have run are up.
    #[test]
    fn taking_the_cloak_plate_off_ends_the_cloak_and_says_so_once() {
        let (mut app, player) = alone_and_wounded(13, 0);
        let plate = crate::testing::equip_new(&mut app, player, "cloak plate");
        app.world_mut().get_mut::<Consumable>(plate).unwrap().left = 1;
        app.world_mut().write_message(Intent::new(player, UseItem(plate)));
        crate::testing::settle(&mut app);
        let fg = |app: &App| app.world().get::<Glyph>(player).map(|g| g.fg);
        assert!(app.world().get::<Unseen>(player).is_some(), "cloaked");
        assert_eq!(fg(&app), Some(crate::run::COMMANDO_UNSEEN));

        crate::testing::unequip(&mut app, player, plate);
        assert!(app.world().get::<Unseen>(player).is_none(), "the plate is off, and so is the cloak");
        assert_eq!(fg(&app), Some(crate::run::COMMANDO), "drawn white again");
        crate::testing::pass_turns(&mut app, 12);
        let said = app.world().resource::<rl_engine::rl_ui::MessageLog>().iter().filter(|e| e.text == "You are no longer cloaked.").count();
        assert_eq!(said, 1, "said once, when it came off");
    }

    /// The exploit held statuses close: cloak with the cloak plate, then
    /// put the nanite plate on over it, displacing the cloak plate from the
    /// same torso slot. Without holding, the commando would keep both, seen
    /// as neither: still cloaked and now mending. Held, the swap is the
    /// same as taking the plate off, in the same update, and the log says
    /// so once.
    #[test]
    fn swapping_the_cloak_plate_for_the_nanite_plate_ends_the_cloak_and_says_so_once() {
        let (mut app, player) = alone_and_wounded(13, 0);
        let plate = crate::testing::equip_new(&mut app, player, "cloak plate");
        app.world_mut().get_mut::<Consumable>(plate).unwrap().left = 1;
        app.world_mut().write_message(Intent::new(player, UseItem(plate)));
        crate::testing::settle(&mut app);
        assert!(app.world().get::<Unseen>(player).is_some(), "cloaked");

        crate::testing::equip_new(&mut app, player, "nanite plate");
        assert!(app.world().get::<Unseen>(player).is_none(), "the nanite plate displaced it, and the commando is seen again in that update");
        crate::testing::pass_turns(&mut app, 12);
        let said = app.world().resource::<rl_engine::rl_ui::MessageLog>().iter().filter(|e| e.text == "You are no longer cloaked.").count();
        assert_eq!(said, 1, "said once, when the nanite plate came on over it");
    }

    /// A stim is no attack and leaves a cloaked commando unseen; a frag
    /// grenade thrown at a droid is one, and the commando is seen again in
    /// the pass it bursts, through the same throw the pack's key makes.
    #[test]
    fn a_stim_leaves_the_commando_cloaked_and_a_grenade_thrown_at_a_droid_ends_it() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (droid, player) = crate::testing::droid_down_a_lane(&mut app, "line droid", 4, 4);
        let frags = carry(&mut app, player, "frag grenade", 1);
        crate::testing::pick(&mut app, player, crate::upgrades::Upgrade::Stims);
        let stims = app.world().resource::<Abilities>().expect("stims");
        let cloaked = app.world().resource::<Registries>().statuses.expect("cloaked");
        app.world_mut().write_message(Afflict { target: player, status: cloaked, turns: 10, by: None, held_by: None });
        crate::testing::pass_turns(&mut app, 1);
        assert!(app.world().get::<Unseen>(player).is_some(), "cloaked");

        let here = at(&app, player);
        app.world_mut().write_message(Intent::new(player, Use { ability: stims, aim: here }));
        crate::testing::settle(&mut app);
        let cooling = app.world().get::<Cooldowns>(player).map(|c| c.ready_at(stims));
        assert!(cooling.is_some_and(|t| t > 0), "the stim was taken, so it is cooling: {cooling:?}");
        assert!(app.world().get::<Unseen>(player).is_some(), "a stim in the arm is not an attack");

        let aim = at(&app, droid);
        throw_grenade(&mut app, player, frags, aim);
        assert!(app.world().get::<Unseen>(player).is_none(), "a grenade at a droid is");
    }

    /// What `make` hands back, each thing with the level it was made at,
    /// read off its `ItemLevel`, and its name.
    fn made(armory: &Armory, registries: &Registries, name: &str, count: u32, band: i32, rng: &mut rand::rngs::StdRng) -> Vec<(Option<i32>, String)> {
        let mut world = World::new();
        let mut queue = CommandQueue::default();
        let from = Provenance { found: rl_engine::rl_bevy::Found::Container, band };
        let things = {
            let mut commands = Commands::new(&mut queue, &world);
            armory.make(&mut commands, registries, armory.defs.expect(name), count, from, rng)
        };
        queue.apply(&mut world);
        things.into_iter().map(|e| (world.get::<ItemLevel>(e).map(|x| x.0), world.get::<Name>(e).map(|n| n.as_str().to_string()).unwrap_or_default())).collect()
    }

    /// A cloak plate found at band 10, over a span of seeds, is always
    /// `+2` to `+5`, every one of the four turns up, and it carries its
    /// level on it and in its name.
    #[test]
    fn a_plate_made_at_band_ten_is_plus_two_to_plus_five_and_says_so() {
        let r = crate::content::registries();
        let armory = crate::testing::armory(&r);
        let mut seen = std::collections::BTreeSet::new();
        for seed in 0..200 {
            let mut rng = rand::SeedableRng::seed_from_u64(seed);
            let [(level, name)] = made(&armory, &r, "cloak plate", 1, 10, &mut rng).try_into().expect("one plate");
            let level = level.expect("a thing that takes levels carries its level");
            assert!((2..=5).contains(&level), "seed {seed}: +{level} at band 10");
            assert_eq!(name, format!("cloak plate +{level}"));
            seen.insert(level);
        }
        assert_eq!(seen.into_iter().collect::<Vec<_>>(), vec![2, 3, 4, 5], "every level the row names turns up");
    }

    /// Two plates from one `make` are two finds, each rolled for itself.
    #[test]
    fn two_plates_made_together_roll_their_levels_apart() {
        let r = crate::content::registries();
        let armory = crate::testing::armory(&r);
        let differ = (0..50).any(|seed| {
            let mut rng = rand::SeedableRng::seed_from_u64(seed);
            let pair = made(&armory, &r, "cloak plate", 2, 10, &mut rng);
            pair[0].0 != pair[1].0
        });
        assert!(differ, "fifty pairs and never two levels: the pair shares one roll");
    }

    /// A thing's `max` caps what the table hands it: a plate that reaches
    /// only `+2` is never found better, even where the row runs to `+5`.
    #[test]
    fn a_things_max_caps_the_level_it_is_made_at() {
        let r = crate::content::registries();
        let mut armory = crate::testing::armory(&r);
        let cloak = armory.defs.expect("cloak plate");
        let mut parts = (**armory.parts(cloak)).clone();
        parts.level = parts.level.map(|l| Level { max: Some(2), ..l });
        armory.parts[cloak.index()] = Arc::new(parts);
        let levels: Vec<i32> = (0..100)
            .map(|seed| {
                let mut rng = rand::SeedableRng::seed_from_u64(seed);
                made(&armory, &r, "cloak plate", 1, 10, &mut rng)[0].0.expect("a level")
            })
            .collect();
        assert!(levels.iter().all(|l| *l == 2), "capped at +2: {levels:?}");
    }

    /// A helmet found on deck ten, where the row runs `+2` to `+5`, is
    /// made at its own `max` of `+3` whenever the row hands it more: the
    /// file's helmet, not a test's.
    #[test]
    fn a_helmet_found_where_the_row_runs_past_its_max_is_made_at_its_max() {
        let r = crate::content::registries();
        let armory = crate::testing::armory(&r);
        let levels: std::collections::BTreeSet<i32> = (0..100)
            .map(|seed| {
                let mut rng = rand::SeedableRng::seed_from_u64(seed);
                made(&armory, &r, "commando helmet", 1, 10, &mut rng)[0].0.expect("a helmet takes levels")
            })
            .collect();
        assert_eq!(levels.into_iter().collect::<Vec<_>>(), vec![2, 3], "+2, and every +3 to +5 the row hands it made at +3");
    }

    /// A stack is made plain and draws nothing from the stream, so a
    /// scatter of slugs and stims does not shift what the plate beside it
    /// rolls.
    #[test]
    fn a_stacking_thing_is_made_without_drawing_from_the_stream() {
        let r = crate::content::registries();
        let armory = crate::testing::armory(&r);
        let (mut a, mut b): (rand::rngs::StdRng, rand::rngs::StdRng) = (rand::SeedableRng::seed_from_u64(4), rand::SeedableRng::seed_from_u64(4));
        let slugs = made(&armory, &r, "slug", 2, 10, &mut a);
        let stims = made(&armory, &r, "stim", 3, 10, &mut a);
        assert!(slugs.iter().chain(&stims).all(|(level, _)| level.is_none()), "plain: {slugs:?} {stims:?}");
        assert_eq!(rand::Rng::random::<u64>(&mut a), rand::Rng::random::<u64>(&mut b), "the stream is where it was");
    }

    /// A level past a thing's `max`, such as one a save wrote before the
    /// file lowered it, comes back at the most the file allows.
    #[test]
    fn a_level_past_max_is_spawned_at_max() {
        let r = crate::content::registries();
        let armory = crate::testing::armory(&r);
        let mut world = World::new();
        let mut queue = CommandQueue::default();
        let plate = {
            let mut commands = Commands::new(&mut queue, &world);
            spawn_item_at(&mut commands, &armory, armory.defs.expect("cloak plate"), 12)
        };
        queue.apply(&mut world);
        assert_eq!(world.get::<ItemLevel>(plate).map(|l| l.0), Some(9));
        assert_eq!(world.get::<Name>(plate).map(|n| n.as_str().to_string()), Some("cloak plate +9".to_string()));
    }

    /// Everything about a spawned thing that a level can change: its
    /// armor, its blow, its shot and its thrown strike, its dark sight,
    /// its clock, and the bonus it carries for its effects, if any.
    #[derive(Debug, Default, PartialEq, Eq)]
    struct Leveled {
        armor: Option<i32>,
        blow: Option<String>,
        shot: Option<String>,
        thrown: Option<String>,
        dark_sight: Option<i32>,
        pulse: Option<u32>,
        bonus: Option<EffectBonus>,
    }

    /// `name` spawned at `level` into a world of its own, and what it
    /// carries that a level can change.
    fn leveled(armory: &Armory, name: &str, level: i32) -> Leveled {
        let mut world = World::new();
        let mut queue = CommandQueue::default();
        let e = {
            let mut commands = Commands::new(&mut queue, &world);
            spawn_item_at(&mut commands, armory, armory.defs.expect(name), level)
        };
        queue.apply(&mut world);
        Leveled {
            armor: world.get::<Armor>(e).map(|a| a.0),
            blow: world.get::<MeleeAttack>(e).map(|m| m.dice.to_string()),
            shot: world.get::<RangedAttack>(e).map(|r| r.dice.to_string()),
            thrown: world.get::<Throwable>(e).and_then(|t| t.strike).map(|(_, dice)| dice.to_string()),
            dark_sight: world.get::<WornDarkSight>(e).map(|d| d.0),
            pulse: world.get::<Pulse>(e).map(|p| p.every),
            bonus: world.get::<EffectBonus>(e).copied(),
        }
    }

    /// Every item at `+0` is what it was before the level block said
    /// what a level does: the table is `items.ron` as it stood then, so a
    /// plain thing found on deck one is the same thing it always was.
    #[test]
    fn every_item_at_plus_nought_is_what_it_was_before_levels_reached_it() {
        let r = crate::content::registries();
        let armory = crate::testing::armory(&r);
        let plain = Leveled::default;
        let blow = |d: &str| Leveled { blow: Some(d.into()), ..plain() };
        let shot = |d: &str| Leveled { shot: Some(d.into()), ..plain() };
        let armor = |a: i32| Leveled { armor: Some(a), ..plain() };
        let table: Vec<(&str, Leveled)> = vec![
            ("monoblade", Leveled { thrown: Some("1d6".into()), ..blow("1d6+1") }),
            ("mono-axe", blow("2d6+1")),
            ("hand blaster", shot("1d6")),
            ("blaster carbine", shot("1d8+1")),
            ("ion pistol", shot("1d6")),
            ("slug pistol", shot("1d8")),
            ("slug rifle", shot("1d10+1")),
            ("heavy repeater", shot("1d8+2")),
            ("slug", plain()),
            ("keycard", plain()),
            ("stim", plain()),
            ("medkit", plain()),
            ("frag grenade", plain()),
            ("smoke grenade", plain()),
            ("ion grenade", plain()),
            ("incendiary grenade", plain()),
            ("commando helmet", armor(1)),
            ("rangefinder helmet", Leveled { dark_sight: Some(6), ..armor(1) }),
            ("scrap plate", armor(1)),
            ("composite plate", armor(2)),
            ("combat gauntlets", armor(1)),
            ("armored greaves", armor(1)),
            ("nanite plate", Leveled { pulse: Some(1000), ..armor(1) }),
            ("cloak plate", armor(1)),
        ];
        assert_eq!(table.len(), armory.defs.len(), "every definition in the table");
        for (name, was) in table {
            assert_eq!(leveled(&armory, name, 0), was, "{name} at +0");
        }
    }

    /// A level adds what the level block says, and armor and damage by
    /// default: plate its armor, a weapon its blow, its shot and its
    /// thrown strike alike, a heavy weapon twice as fast, and a helmet's
    /// dark sight only because its block says so.
    #[test]
    fn a_level_adds_to_armor_damage_and_dark_sight_as_the_level_block_says() {
        let r = crate::content::registries();
        let armory = crate::testing::armory(&r);
        let at = |name, level| leveled(&armory, name, level);
        assert_eq!(at("composite plate", 3).armor, Some(5), "+1 armor a level, inferred");
        assert_eq!(at("mono-axe", 2).blow.as_deref(), Some("2d6+5"), "damage: 2, so +4 at +2");
        let blade = at("monoblade", 2);
        assert_eq!((blade.blow.as_deref(), blade.thrown.as_deref()), (Some("1d6+3"), Some("1d6+2")), "the blow and the throw both gain 2");
        assert_eq!(at("slug rifle", 2).shot.as_deref(), Some("1d10+5"), "damage: 2");
        assert_eq!(at("hand blaster", 2).shot.as_deref(), Some("1d6+2"), "+1 damage a level, inferred");
        assert_eq!(at("rangefinder helmet", 2).dark_sight, Some(8), "dark_sight: 1, so two more at +2");
        assert_eq!(at("rangefinder helmet", 2).armor, Some(3), "and its armor by the default");
        let cloak = at("cloak plate", 2);
        assert_eq!(cloak.armor, Some(1), "armor: 0 keeps it at 1");
        assert_eq!(cloak.bonus, Some(EffectBonus { turns: 4, amount: 0 }), "turns: 2, so four more on its effects at +2");
    }

    /// Every `NoiseHeard`, recorded as it is written: a headless app
    /// rotates its message buffers on wall time.
    #[derive(Resource, Default)]
    struct Earful(Vec<(Entity, SoundId)>);

    fn record_heard(mut heard: MessageReader<NoiseHeard>, mut out: ResMut<Earful>) {
        out.0.extend(heard.read().map(|h| (h.listener, h.sound)));
    }

    /// Whether a line droid `near` steps and one `far` steps past where a
    /// thrown `grenade` lands, down one straight lane of open deck, each
    /// heard the `sound` it makes. Nothing is in the burst, so nothing is
    /// struck and the grenade's own sound is the only one.
    ///
    /// Thrown on deck two with the shoulder lamp off, so it lands in the
    /// dark where the commando cannot see it: what the droids learn of it
    /// they learn by ear, never because anyone watched it land.
    fn heard_down_the_lane(grenade: &str, sound: &str, near: i32, far: i32) -> (bool, bool) {
        let mut app = crate::testing::headless(RunSeed(1));
        crate::testing::arrive_on(&mut app, 2);
        crate::testing::clear_droids(&mut app, &[]);
        let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
        app.world_mut().entity_mut(player).remove::<LightSource>();
        let from = at(&app, player);
        // Thrown three steps down the lane, and the lane run past both
        // droids, so each is exactly as many steps from the landing as
        // it is cells.
        let aim = from.offset(3, 0);
        let floor = app.world().resource::<WorldMap>().tile(from).expect("the commando's tile is loaded");
        {
            let mut map = app.world_mut().resource_mut::<WorldMap>();
            for dx in 1..=3 + far {
                map.set_tile(from.offset(dx, 0), floor);
            }
            // Or the far droid's silence would be a lane off the deck.
            assert_eq!(map.tile(aim.offset(far, 0)), Some(floor), "the lane reaches the far droid");
        }
        // And nothing stands in it: a room's machinery on the lane would
        // stop the throw or lengthen the walk the sound takes.
        let lane: Vec<Point> = (1..=3 + far).map(|dx| from.offset(dx, 0)).collect();
        let in_lane: Vec<Entity> = app
            .world_mut()
            .query_filtered::<(Entity, &Position), With<Prop>>()
            .iter(app.world())
            .filter(|(_, p)| lane.contains(&p.0))
            .map(|(e, _)| e)
            .collect();
        for prop in in_lane {
            app.world_mut().despawn(prop);
        }
        let registries = app.world().resource::<Registries>().clone();
        let roster = crate::droids::Roster::load(&registries);
        let deck = app.world().resource::<WorldMap>().current();
        let droids = {
            let mut queue = CommandQueue::default();
            let mut commands = Commands::new(&mut queue, app.world_mut());
            let line = roster.defs.expect("line droid");
            let droids = [near, far].map(|d| crate::droids::spawn_monster(&mut commands, &roster, line, aim.offset(d, 0), deck, &registries));
            queue.apply(app.world_mut());
            droids
        };
        let grenade = carry(&mut app, player, grenade, 1);
        let sight = app.world().get::<Viewshed>(player).expect("the commando sees");
        assert!(sight.in_line(aim) && !sight.can_see(aim), "the landing is in the commando's line and too dark to see");
        app.init_resource::<Earful>().add_systems(PostUpdate, record_heard);
        throw_grenade(&mut app, player, grenade, aim);
        crate::testing::pass_turns(&mut app, 1);
        let sound = app.world().resource::<Sounds>().get(sound).expect("Foundry declares it");
        let earful = &app.world().resource::<Earful>().0;
        let heard = |droid: Entity| earful.contains(&(droid, sound));
        (heard(droids[0]), heard(droids[1]))
    }

    /// A grenade thrown into the dark is heard where it lands: a blast by
    /// a droid fourteen steps off and not by one a step further, smoke's
    /// hiss six steps off and not seven. The commando threw it, so the
    /// commando does not hear it: the engine never tells a noise to its
    /// own maker, and the noise meter reads only what the commando hears.
    #[test]
    fn a_grenade_thrown_out_of_sight_is_heard_as_far_as_its_noise_carries_and_no_further() {
        assert_eq!(heard_down_the_lane("frag grenade", "blast", 14, 15), (true, false), "a blast carries fourteen steps");
        assert_eq!(heard_down_the_lane("smoke grenade", "hiss", 6, 7), (true, false), "a hiss carries six");
    }

    /// An armory loaded with no app, as `foundry --prefabs` and a test
    /// with no world load one, names every sound by the id the app gives
    /// it, so a grenade's `blast` is the same sound in either.
    #[test]
    fn the_sounds_built_with_no_app_have_the_ids_the_app_gives_them() {
        let app = crate::testing::headless(RunSeed(1));
        let (theirs, mine) = (app.world().resource::<Sounds>(), sounds());
        for name in Sounds::BUILT_IN.into_iter().chain(SOUNDS) {
            assert!(mine.get(name).is_some(), "{name} is declared with no app");
            assert_eq!(mine.get(name), theirs.get(name), "{name} has one id with an app and without");
        }
    }

    #[test]
    fn a_weapon_naming_both_heat_and_ammo_fails_to_validate() {
        let r = crate::content::registries();
        let armory = crate::testing::armory(&r);
        let mut d = blank_def("double economy");
        d.heat = Some((10, 10));
        d.ammo = Some(rl_engine::rl_core::Id::from_raw(0).into());
        assert!(validate_def(&d, &armory.defs).is_err(), "heat and ammo on the same weapon");
    }
}
