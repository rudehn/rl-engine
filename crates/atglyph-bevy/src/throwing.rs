//! Throwing: a carried item flies at a cell, strikes whoever it meets
//! first, and comes to rest.
//!
//! Its own plugin, because a throw is both an item and a blow: the item
//! leaves a bag the way a dropped one does, and whoever it strikes is hurt
//! down the same damage pipeline a sword's blow goes down. So neither
//! [`ItemsPlugin`](crate::items::ItemsPlugin) nor
//! [`CombatPlugin`](crate::combat::CombatPlugin) has to know the other
//! exists.
//!
//! [`flight`] is the one answer to where a throw goes. The resolver throws
//! with it and the targeting cursor previews with it, so the cells a player
//! is shown are the cells the item will fly through.

use crate::app::EngineApp;
use atglyph_core::turn::BASE_ACTION_COST;
use atglyph_core::{DiceRoll, Point};
use atglyph_grid::{Footprint, TargetMode};
use atglyph_rules::Hit;
use atglyph_rules::damage::DamageKindId;
use bevy::prelude::*;

use crate::combat::{CombatRng, DamageEvent, Dead, Health};
use crate::components::{MyTurn, Position};
use crate::cue::{AddAirborne, Airborne, Anchor, Cue, CueWait, Cued, LookOf};
use crate::effects::{Fired, TriggerKinds};
use crate::items::{Equipped, Inventory, Item, ItemEvent, Stack, Tagged};
use crate::places::{MapId, OnMap};
use crate::turn::{Action, Intent, Occupancy, Resolution};
use crate::world::WorldMap;
use atglyph_rules::{Equipment, TagId};

/// An item made to be thrown: how far it goes, and what it does to whoever
/// it strikes.
///
/// Items without one cannot be thrown. A flask that should only shatter
/// where it lands has no strike, and the game answers
/// [`ItemEvent::Thrown`] to spill whatever was in it. How far one actually
/// flies from a given thrower is [`Throws::of`]'s to say, since a thing
/// with no range of its own goes as far as its thrower reaches.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Throwable {
    /// The furthest cell it reaches whoever throws it, or `None` to fly as
    /// far as its thrower's reach, [`CombatRules::reach`](crate::combat::CombatRules::reach).
    pub range: Option<i32>,
    /// The kind of damage and the roll it deals whoever it strikes, if any.
    pub strike: Option<(DamageKindId, DiceRoll)>,
    /// The furthest cell with no range penalty, for a hit model that reads
    /// one. `None` is a third of how far it flies.
    pub effective: Option<i32>,
}

impl Throwable {
    /// Reaching `range`, or its thrower's reach for `None`, and striking
    /// with `strike`, its effective range a third of how far it flies. A
    /// constructor rather than a literal, so a field only some games want
    /// is added without touching every call site.
    pub const fn new(range: Option<i32>, strike: Option<(DamageKindId, DiceRoll)>) -> Self {
        Self { range, strike, effective: None }
    }

    /// The throw `part` describes: the authored throw made a component.
    pub fn from_part(part: &atglyph_rules::parts::ThrowDef) -> Self {
        Self { effective: part.effective, ..Self::new(part.range, part.strike.map(|(roll, kind)| (kind.id(), roll))) }
    }

    /// No range penalty out to `cells`.
    pub const fn effective_to(mut self, cells: i32) -> Self {
        self.effective = Some(cells);
        self
    }
}

/// What a worn thing launches: the things tagged `ammo`, flown `range`
/// cells further than their thrower reaches and striking for `damage`
/// more, rolled beside their own roll.
///
/// On a wearable thing, and only worn does it launch anything, so a
/// launcher in the pack is a thing carried. A thrown thing it launches
/// flies, strikes and lands as any throw does, and can be picked up
/// where it comes down.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Launcher {
    /// What it launches.
    pub ammo: TagId,
    /// The cells it adds to the thrower's reach, or to a thing's own range.
    pub range: u32,
    /// The roll it adds to whatever the thing launched strikes for.
    pub damage: DiceRoll,
}

impl Launcher {
    /// The launcher `part` describes: the authored launcher made a component.
    pub fn from_part(part: &atglyph_rules::parts::LaunchDef) -> Self {
        Self { ammo: part.ammo.id(), range: part.range, damage: part.damage }
    }
}

/// A thing was launched rather than thrown by hand: written beside its
/// [`ItemEvent::Thrown`], so whatever tells of throws can tell a shot.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Launched {
    /// Who launched it.
    pub actor: Entity,
    /// What was launched, as it lies where it came down.
    pub item: Entity,
    /// What launched it.
    pub launcher: Entity,
}

/// A throw as it flies from one thrower: how far, how far before its aim
/// falls off, what it strikes with, and what launches it.
///
/// What every reader of a throw reads rather than the item's
/// [`Throwable`], so the aim, the throw itself, the odds and every mind
/// weighing a missile agree on it: [`Throws::of`] makes one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThrowAs {
    /// The furthest cell it reaches.
    pub range: i32,
    /// The furthest cell with no range penalty.
    pub effective: i32,
    /// The kind of damage and the roll it deals whoever it strikes, if any.
    pub strike: Option<(DamageKindId, DiceRoll)>,
    /// The worn [`Launcher`] that launches it and the roll it adds to the
    /// strike, when one does.
    pub launched: Option<(Entity, DiceRoll)>,
}

/// How `own` flies from a thrower who reaches `reach`, launched by
/// `launcher` when one is worn for it, if it can be thrown: its own range,
/// or the reach for a thing with none, plus the launcher's; its effective
/// range its own, or a third of how far it flies. `None` for a thing with
/// no range thrown by someone with no reach, which nobody can throw.
pub fn resolve(own: &Throwable, reach: Option<i32>, launcher: Option<(Entity, &Launcher)>) -> Option<ThrowAs> {
    let range = own.range.or(reach)? + launcher.map_or(0, |(_, l)| l.range as i32);
    let launched = launcher.map(|(e, l)| (e, l.damage));
    Some(ThrowAs { range, effective: own.effective.unwrap_or(range / 3), strike: own.strike, launched })
}

/// How a thrower throws a thing, given what it wears: the rules
/// [`Throws`] answers by, for a system that already holds what is worn,
/// as the throw resolver does.
#[derive(bevy::ecs::system::SystemParam)]
pub struct ThrowRules<'w, 's> {
    combat: Option<Res<'w, crate::combat::CombatRules>>,
    registries: Option<Res<'w, crate::registries::Registries>>,
    stats: Query<'w, 's, &'static crate::status::StatBlock>,
    missiles: Query<'w, 's, &'static Throwable>,
    launchers: Query<'w, 's, &'static Launcher>,
    tagged: Query<'w, 's, &'static Tagged>,
}

impl ThrowRules<'_, '_> {
    /// How far `thrower` throws a thing with no range of its own, or `None`
    /// when the game gives throws no reach.
    pub fn reach(&self, thrower: Entity) -> Option<i32> {
        let rule = self.combat.as_ref()?.reach?;
        let stat = rule.stat.and_then(|stat| Some(self.stats.get(thrower).ok()?.value(stat, &self.registries.as_ref()?.stats)));
        Some(rule.base + stat.unwrap_or(0))
    }

    /// How `item` flies when `thrower`, wearing `worn`, throws it: launched
    /// by the first worn launcher, in slot order, for one of its tags.
    pub fn by(&self, thrower: Entity, item: Entity, worn: Option<&Equipment<Entity>>) -> Option<ThrowAs> {
        let tags = self.tagged.get(item).map(|t| t.0.as_slice()).unwrap_or_default();
        let launcher = worn.and_then(|w| w.worn().find_map(|(_, e)| self.launchers.get(e).ok().filter(|l| tags.contains(&l.ammo)).map(|l| (e, l))));
        resolve(self.missiles.get(item).ok()?, self.reach(thrower), launcher)
    }
}

/// The one answer to how far and how hard a thrower throws a thing.
///
/// A parameter rather than a function of the item, because the answer
/// depends on who throws: its reach, by [`CombatRules::reach`](crate::combat::CombatRules::reach),
/// is a base plus one of its stats, and a [`Launcher`] it wears adds to
/// what it launches.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Throws<'w, 's> {
    rules: ThrowRules<'w, 's>,
    worn: Query<'w, 's, &'static Equipped>,
}

impl Throws<'_, '_> {
    /// How far `thrower` throws a thing with no range of its own, or `None`
    /// when the game gives throws no reach.
    pub fn reach(&self, thrower: Entity) -> Option<i32> {
        self.rules.reach(thrower)
    }

    /// How `item` flies when `thrower` throws it, or `None` when it cannot
    /// throw it: the item is not [`Throwable`], or has no range and the
    /// game gives throws no reach.
    pub fn of(&self, thrower: Entity, item: Entity) -> Option<ThrowAs> {
        self.rules.by(thrower, item, self.worn.get(thrower).ok().map(|w| &w.0))
    }
}

/// Throw a carried item at a cell.
///
/// At a cell rather than at an actor, for the reason an ability is: a throw
/// goes where it is aimed whether or not anyone is standing there. One from
/// a stack is thrown and the rest stay in the bag; a worn item is taken off
/// first. Refused when the item is not carried, cannot be thrown, or is
/// aimed at the thrower's own feet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Throw {
    /// What is thrown.
    pub item: Entity,
    /// Where it is aimed.
    pub at: Point,
}
impl Action for Throw {}

/// Where a throw goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flight {
    /// The cells it flies through, the one it comes to rest in last.
    pub path: Vec<Point>,
    /// Whoever it meets first, if it meets anyone.
    pub struck: Option<Entity>,
    /// Where it comes to rest: at the feet of whoever it struck, short of a
    /// wall it hit, or at the end of its flight.
    pub rests: Point,
}

/// Where an item thrown from `from` at `at`, reaching `range`, goes: the
/// flight a bolt takes, as [`shot`](crate::combat::shot) answers it, so a
/// thrown thing and a spell agree about what is in the way and where they
/// come to rest.
pub fn flight(map: &WorldMap, occupancy: &Occupancy, from: Point, at: Point, range: i32) -> Flight {
    let Footprint { path, reached, landing, .. } = crate::combat::projectile(map, occupancy, TargetMode::Bolt { range }, from, at);
    let struck = reached.and_then(|end| occupancy.first_at(end));
    Flight { path, struck, rests: landing.unwrap_or(from) }
}

/// What a throw reads and moves.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Launch<'w, 's> {
    map: Res<'w, WorldMap>,
    occupancy: Res<'w, Occupancy>,
    rng: ResMut<'w, CombatRng>,
    throwers: Query<'w, 's, (&'static Position, &'static mut Inventory, Option<&'static mut Equipped>), With<MyTurn>>,
    missiles: Query<'w, 's, (&'static Throwable, Option<&'static mut Stack>), With<Item>>,
    alive: Query<'w, 's, (), (With<Health>, Without<Dead>)>,
    cues: MessageWriter<'w, Cued>,
    cue_wait: ResMut<'w, CueWait>,
    airborne: ResMut<'w, Airborne<ThrowLanding>>,
    /// The odds of striking whoever the flight meets.
    marks: crate::accuracy::Marksmanship<'w, 's>,
    /// How far and how hard the thrower throws it, given what it wears.
    throws: ThrowRules<'w, 's>,
}

/// A throw that has left the hand and not yet come down: what it will do
/// when it does.
#[derive(Debug, Clone)]
pub(crate) struct ThrowLanding {
    actor: Entity,
    item: Entity,
    rests: Point,
    struck: Option<Entity>,
    strike: Option<(DamageKindId, DiceRoll)>,
    /// What launched it and the roll that adds, when something did.
    launched: Option<(Entity, DiceRoll)>,
    /// The odds against whoever it struck, read as it left the hand and
    /// rolled as it lands, where the damage is rolled.
    odds: Option<atglyph_rules::Odds>,
}

impl crate::cue::Lands for ThrowLanding {}

/// Resolves [`Throw`] for the actor holding the turn: the flight, the one
/// that leaves the hand, the blow if it strikes anyone, and where it lands.
///
/// A worn thing thrown comes off, so a [`Stuck`](crate::items::Stuck) one
/// is refused, as any impossible throw is, and said as
/// [`ItemEvent::WouldNotComeOff`].
pub(crate) fn resolve_throws(
    mut commands: Commands,
    mut intents: MessageReader<Intent<Throw>>,
    mut resolution: Resolution,
    launch: Launch,
    mut reports: ThrowReports,
    stuck: Query<(), With<crate::items::Stuck>>,
) {
    let Launch { map, occupancy, mut rng, mut throwers, mut missiles, alive, mut cues, mut cue_wait, mut airborne, marks, throws } = launch;
    for intent in intents.read() {
        let (actor, Throw { item, at }) = (intent.actor, intent.action);
        if !resolution.claim(actor) {
            continue;
        }
        let thrown_as = throwers.get(actor).ok().and_then(|(_, _, worn)| throws.by(actor, item, worn.map(|w| &w.0)));
        let (Ok((pos, mut bag, worn)), Ok((_, stack)), Some(thrown_as)) = (throwers.get_mut(actor), missiles.get_mut(item), thrown_as) else {
            resolution.failed(actor, BASE_ACTION_COST);
            continue;
        };
        if !bag.contains(item) || at == pos.0 {
            resolution.failed(actor, BASE_ACTION_COST);
            continue;
        }
        if stuck.contains(item) && worn.as_ref().is_some_and(|w| w.contains(item)) {
            reports.events.write(ItemEvent::WouldNotComeOff { actor, item });
            resolution.failed(actor, BASE_ACTION_COST);
            continue;
        }
        let ThrowAs { range, strike, launched, .. } = thrown_as;
        let Flight { struck, rests, .. } = flight(&map, &occupancy, pos.0, at, range);

        // One leaves the hand: off the top of a stack, which makes it a
        // thing of its own, or the item itself.
        let thrown = match stack {
            Some(stack) if stack.count > 1 => crate::items::split_off(&mut commands, item, *stack, 1),
            _ => {
                bag.remove(item);
                if let Some(mut worn) = worn
                    && worn.unequip(item)
                {
                    reports.events.write(ItemEvent::Unequipped { actor, item });
                }
                item
            }
        };
        // The flight, in the item's own glyph, landing on whoever it struck
        // wherever they are by the time it is seen.
        let struck = struck.filter(|who| alive.contains(*who));
        let odds = struck.and_then(|who| marks.odds(actor, who, crate::accuracy::Attempt::Throw(&thrown_as), 0));
        let to = struck.map(|who| Anchor::on(who, rests)).unwrap_or(Anchor::cell(rests));
        cues.write(Cued { actor, cue: Cue::Flight { from: Anchor::on(actor, pos.0), to, look: LookOf::Item(thrown) } });
        let landing = ThrowLanding { actor, item: thrown, rests, struck, strike, launched, odds };
        resolution.done(actor, BASE_ACTION_COST);
        // The turn is spent on the throw. With something watching, the
        // knife is in the air until its flight has been seen, and lies
        // nowhere until it comes down.
        let Some(landing) = airborne.launched(&mut cue_wait, landing) else { continue };
        land(landing, map.current(), &mut commands, &mut rng, &alive, &mut reports);
    }
}

/// What a throw reports: the blow it strikes, the item's own event, and
/// the `land` moment for whatever its triggers do where it comes down.
#[derive(bevy::ecs::system::SystemParam)]
pub struct ThrowReports<'w> {
    damage: MessageWriter<'w, DamageEvent>,
    events: MessageWriter<'w, ItemEvent>,
    fired: MessageWriter<'w, Fired>,
    missed: MessageWriter<'w, crate::accuracy::Missed>,
    launched: MessageWriter<'w, Launched>,
}

/// Lands every throw in the air, on the first pass after its flight has
/// been seen. Landing counts as progress, so the loop goes on to deal the
/// next turn.
pub(crate) fn land_throws(mut commands: Commands, launch: Launch, mut reports: ThrowReports, mut turns: ResMut<crate::turn::Turns>) {
    let Launch { map, mut rng, alive, mut cue_wait, mut airborne, .. } = launch;
    for landing in airborne.landing(&mut cue_wait, &mut turns) {
        land(landing, map.current(), &mut commands, &mut rng, &alive, &mut reports);
    }
}

/// Where the thrown thing comes down: on the ground where it rests, into
/// whoever it struck, and as the `land` moment. A consumable one is spent
/// by that moment and gone before anyone could pick it up; anything else
/// lies where it came to rest.
fn land(
    landing: ThrowLanding,
    map: MapId,
    commands: &mut Commands,
    rng: &mut CombatRng,
    alive: &Query<(), (With<Health>, Without<Dead>)>,
    reports: &mut ThrowReports,
) {
    let ThrowLanding { actor, item, rests, struck, strike, launched, odds } = landing;
    commands.entity(item).insert((Position(rests), OnMap(map)));
    let target = struck.filter(|who| alive.contains(*who));
    // Rolled where the damage is rolled, as it lands, so a watched throw
    // and an unwatched one draw in the same order. A miss reports whom it
    // missed and then lands as a throw that struck nobody.
    let missed = target.is_some() && odds.is_some_and(|o| !o.roll(&mut **rng));
    if let (true, Some(target)) = (missed, target) {
        reports.missed.write(crate::accuracy::Missed { attacker: actor, target, with: Some(item), reach: crate::combat::DamageRoute::Thrown });
    }
    let struck = if missed { None } else { target };
    if let (Some(target), Some((kind, dice))) = (struck, strike) {
        // Floored where it is rolled, as a blow is: a throw that rolls
        // below nothing has missed, not healed.
        let extra = launched.map_or(0, |(_, roll)| roll.roll(&mut **rng));
        let amount = (dice.roll(&mut **rng) + extra).max(0);
        reports.damage.write(DamageEvent::arriving(target, Hit::by(actor, kind, amount), crate::combat::DamageRoute::Thrown));
    }
    reports.events.write(ItemEvent::Thrown { actor, item, at: Position(rests), struck });
    if let Some((launcher, _)) = launched {
        reports.launched.write(Launched { actor, item, launcher });
    }
    reports.fired.write(Fired { on: item, moment: TriggerKinds::LAND, by: Some(actor), at: rests });
}

/// Throwing: [`Throw`], resolved beside every other action, down combat's
/// damage pipeline.
///
/// Needs [`ItemsPlugin`](crate::items::ItemsPlugin) for the bag a throw
/// leaves and [`CombatPlugin`](crate::combat::CombatPlugin) for the blow it
/// strikes. Minds throw when their wits allow and their brain holds
/// [`ThrowAtRange`](atglyph_rules::ai::tactics::ThrowAtRange).
pub struct ThrowingPlugin;

impl Plugin for ThrowingPlugin {
    fn build(&self, app: &mut App) {
        use crate::app::EngineApp;
        use crate::plugin::{ResolveSet, Turn};
        app.add_airborne::<ThrowLanding>()
            // What a landing reports, for the triggers it sets off. A game may
            // throw and have no effects, and then the queue simply stays empty.
            .add_message::<Fired>()
            .add_message::<Launched>()
            .add_action::<Throw>()
            .add_systems(Turn, (land_throws.in_set(crate::plugin::LandSet::Throw), resolve_throws.in_set(ResolveSet::Act)).chain());
    }

    fn finish(&self, app: &mut App) {
        app.depends_on::<crate::items::ItemsPlugin>("ThrowingPlugin");
        app.depends_on::<crate::combat::CombatPlugin>("ThrowingPlugin");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::{CombatPlugin, Faction};
    use crate::components::{Actor, Blocks, Player, RevealsMap, Viewshed};
    use crate::items::ItemsPlugin;
    use crate::state::EngineState;
    use crate::testing::headless_app;
    use crate::turn::Turns;
    use atglyph_grid::TileId;

    struct Rig {
        app: App,
        player: Entity,
        start: Point,
        sides: crate::testing::TwoSides,
    }

    impl Rig {
        fn new() -> Self {
            let mut app = headless_app();
            app.add_plugins((crate::fov::FovPlugin, CombatPlugin, ItemsPlugin, ThrowingPlugin, crate::effects::EffectsPlugin));
            crate::app::EngineApp::add_engine_effects(&mut app);
            let start = crate::testing::ground(&mut app);
            let sides = crate::testing::two_sides(&mut app);
            let player = app
                .world_mut()
                .spawn((Actor, Player, Blocks, Position(start), Viewshed::new(8), RevealsMap, Health::full(30), Faction(sides.ours), Inventory::default()))
                .id();
            app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
            app.update();
            app.update();
            Self { app, player, start, sides }
        }

        /// A stack of `count` knives in the player's bag.
        fn knives(&mut self, count: u32) -> Entity {
            let knife = Throwable::new(Some(5), Some((self.sides.kind, DiceRoll::flat(3))));
            let stack = self.app.world_mut().spawn((Item, knife, Stack { key: 1, count }, Name::new("knife"))).id();
            self.app.world_mut().get_mut::<Inventory>(self.player).unwrap().items.push(stack);
            stack
        }

        /// A stack of `count` grenades in the player's bag: thrown rather
        /// than struck with, one charge each, gone when spent, and a burst
        /// of four where one comes down.
        fn grenades(&mut self, count: u32) -> Entity {
            let triggers = {
                let specs: Vec<atglyph_rules::TriggerSpec> = atglyph_rules::Names::new()
                    .load_list(r#"[(on: "land", area: Burst(radius: 1), effects: [(kind: "Harm", args: (kind: "kinetic", roll: "4"))])]"#)
                    .expect("the trigger parses");
                let world = self.app.world();
                let registries = world.resource::<crate::registries::Registries>();
                crate::effects::Triggers::build(
                    &specs,
                    &[],
                    world.resource::<crate::effects::TriggerKinds>(),
                    world.resource::<crate::effects::EffectKinds>(),
                    &registries.names(),
                )
                .expect("the trigger builds")
            };
            let spent = crate::consumable::Consumable::new(1, crate::consumable::WhenEmpty::Destroyed);
            let stack = self.app.world_mut().spawn((Item, Throwable::new(Some(6), None), spent, triggers, Stack { key: 2, count }, Name::new("grenade"))).id();
            self.app.world_mut().get_mut::<Inventory>(self.player).unwrap().items.push(stack);
            stack
        }

        /// A stone in the player's bag: no range of its own, so it flies as
        /// far as its thrower reaches, and struck for 2.
        fn stone(&mut self) -> Entity {
            let stone = Throwable::new(None, Some((self.sides.kind, DiceRoll::flat(2))));
            let e = self.app.world_mut().spawn((Item, stone, Name::new("stone"))).id();
            self.app.world_mut().get_mut::<Inventory>(self.player).unwrap().items.push(e);
            e
        }

        /// Throws reach `base` plus a stat, which the player holds at `stat`.
        fn reach(&mut self, base: i32, stat: i32) {
            let stats = atglyph_rules::Registry::from_defs(vec![atglyph_rules::StatDef::new("heft", 0)]).unwrap();
            let heft = stats.expect("heft");
            self.app.world_mut().resource_mut::<crate::registries::Registries>().stats = stats;
            self.app.world_mut().resource_mut::<crate::combat::CombatRules>().reach = Some(crate::combat::ThrowReach { base, stat: Some(heft) });
            let mut block = atglyph_rules::stats::Stats::new();
            block.set_base(heft, stat);
            self.app.world_mut().entity_mut(self.player).insert(crate::status::StatBlock(block));
        }

        /// A missile in the player's bag, tagged `tag`, with no range of its
        /// own and struck for 2.
        fn missile(&mut self, tag: u32) -> Entity {
            let e = self.stone();
            self.app.world_mut().entity_mut(e).insert(crate::items::Tagged(vec![atglyph_rules::TagId::from_raw(tag)]));
            e
        }

        /// A launcher for things tagged `tag`, adding 4 to the reach and 3
        /// to the roll, worn by the player, or only carried.
        fn launcher(&mut self, tag: u32, worn: bool) -> Entity {
            let shape = atglyph_rules::EquipShape::in_slot(atglyph_rules::SlotId::from_raw(0));
            let launcher = Launcher { ammo: atglyph_rules::TagId::from_raw(tag), range: 4, damage: DiceRoll::flat(3) };
            let e = self.app.world_mut().spawn((Item, launcher, crate::items::Wearable(shape.clone()), Name::new("launcher"))).id();
            self.app.world_mut().get_mut::<Inventory>(self.player).unwrap().items.push(e);
            let mut equipment = atglyph_rules::Equipment::with_slot_count(1);
            if worn {
                equipment.equip(e, &shape).unwrap();
            }
            self.app.world_mut().entity_mut(self.player).insert(Equipped(equipment));
            e
        }

        /// Every item lying on `at`.
        fn lying_at(&mut self, at: Point) -> Vec<Entity> {
            let world = self.app.world_mut();
            let mut q = world.query_filtered::<(Entity, &Position), With<Item>>();
            q.iter(world).filter(|(_, p)| p.0 == at).map(|(e, _)| e).collect()
        }

        /// Someone to throw at, `dx` east of the player.
        fn mark(&mut self, dx: i32) -> Entity {
            let at = self.start.offset(dx, 0);
            let theirs = self.sides.theirs;
            let e = self.app.world_mut().spawn((Actor, Blocks, Position(at), Health::full(20), Faction(theirs))).id();
            self.app.update();
            e
        }

        fn throw(&mut self, item: Entity, dx: i32) -> Vec<ItemEvent> {
            let at = self.start.offset(dx, 0);
            self.app.world_mut().write_message(Intent::new(self.player, Throw { item, at }));
            self.app.update();
            self.app.world_mut().resource_mut::<Messages<ItemEvent>>().drain().collect()
        }

        fn hp(&self, who: Entity) -> i32 {
            self.app.world().get::<Health>(who).unwrap().current
        }

        fn now(&self) -> u32 {
            self.app.world().resource::<Turns>().now()
        }
    }

    /// Always misses.
    struct Never;
    impl atglyph_rules::accuracy::HitModel for Never {
        fn odds(&self, _: &atglyph_rules::accuracy::Shot) -> Option<atglyph_rules::Odds> {
            Some(atglyph_rules::Odds::new(0, 100, Vec::new()))
        }
    }

    /// Hits only at exactly this distance, and misses everywhere else.
    struct OnlyAt(i32);
    impl atglyph_rules::accuracy::HitModel for OnlyAt {
        fn odds(&self, shot: &atglyph_rules::accuracy::Shot) -> Option<atglyph_rules::Odds> {
            Some(atglyph_rules::Odds::new(u32::from(shot.distance == self.0), 1, Vec::new()))
        }
    }

    #[test]
    fn a_throw_with_no_range_resolves_to_its_throwers_reach_and_one_with_a_range_keeps_it() {
        let kind = atglyph_rules::damage::DamageKindId::from_raw(0);
        let strike = Some((kind, DiceRoll::flat(1)));
        let rangeless = Throwable::new(None, strike);
        assert_eq!(resolve(&rangeless, Some(6), None), Some(ThrowAs { range: 6, effective: 2, strike, launched: None }));
        assert_eq!(resolve(&rangeless, None, None), None, "no range and no reach: it cannot be thrown");
        let ranged = Throwable::new(Some(9), strike).effective_to(4);
        assert_eq!(resolve(&ranged, Some(6), None), Some(ThrowAs { range: 9, effective: 4, strike, launched: None }));
        assert_eq!(resolve(&ranged, None, None), Some(ThrowAs { range: 9, effective: 4, strike, launched: None }));
    }

    #[test]
    fn a_stone_with_no_range_flies_as_far_as_its_throwers_reach() {
        let mut rig = Rig::new();
        rig.reach(3, 2);
        let near = rig.mark(5);
        let stone = rig.stone();
        rig.throw(stone, 5);
        assert_eq!(rig.hp(near), 18, "five away is in reach");

        let mut rig = Rig::new();
        rig.reach(3, 2);
        let far = rig.mark(6);
        let stone = rig.stone();
        rig.throw(stone, 6);
        assert_eq!(rig.hp(far), 20, "six away is not");
        let short = rig.start.offset(5, 0);
        assert_eq!(rig.lying_at(short), vec![stone], "it came down at the end of its reach");
    }

    #[test]
    fn an_item_with_its_own_range_keeps_it_whatever_its_thrower_reaches() {
        let mut rig = Rig::new();
        rig.reach(9, 0);
        let far = rig.mark(6);
        let knife = rig.knives(1);
        rig.throw(knife, 6);
        assert_eq!(rig.hp(far), 20, "the knife flies five, not nine");
    }

    #[test]
    fn with_no_reach_rule_a_rangeless_throw_is_refused() {
        let mut rig = Rig::new();
        let stone = rig.stone();
        assert!(rig.throw(stone, 2).is_empty(), "nothing thrown");
        assert!(rig.app.world().get::<Inventory>(rig.player).unwrap().contains(stone), "still carried");
    }

    #[test]
    fn a_launcher_adds_its_range_and_roll_and_its_effective_range_is_a_third_of_the_sum() {
        let kind = atglyph_rules::damage::DamageKindId::from_raw(0);
        let strike = Some((kind, DiceRoll::flat(2)));
        let launcher = Launcher { ammo: atglyph_rules::TagId::from_raw(0), range: 4, damage: DiceRoll::new(1, 4) };
        let e = Entity::from_bits(7);
        let thrown = resolve(&Throwable::new(None, strike), Some(5), Some((e, &launcher)));
        assert_eq!(thrown, Some(ThrowAs { range: 9, effective: 3, strike, launched: Some((e, DiceRoll::new(1, 4))) }));
        let own = resolve(&Throwable::new(Some(6), strike).effective_to(5), None, Some((e, &launcher)));
        assert_eq!(own.map(|t| (t.range, t.effective)), Some((10, 5)), "its own range and its own effective range");
    }

    #[test]
    fn a_missile_thrown_by_hand_flies_its_throwers_reach_for_its_own_roll() {
        let mut rig = Rig::new();
        rig.reach(5, 0);
        let near = rig.mark(5);
        let missile = rig.missile(0);
        rig.throw(missile, 5);
        assert_eq!(rig.hp(near), 18);
    }

    #[test]
    fn a_missile_thrown_with_its_launcher_worn_adds_the_launchers_range_and_roll() {
        let mut rig = Rig::new();
        rig.reach(5, 0);
        rig.launcher(0, true);
        let far = rig.mark(9);
        let missile = rig.missile(0);
        rig.throw(missile, 9);
        assert_eq!(rig.hp(far), 15, "nine away, struck for 2 and the launcher's 3");
        assert_eq!(rig.lying_at(rig.start.offset(9, 0)), vec![missile], "and it lies where it struck, to be picked up");
        let launched: Vec<Launched> = rig.app.world_mut().resource_mut::<Messages<Launched>>().drain().collect();
        assert_eq!(launched.len(), 1, "a launch is said as one");
    }

    #[test]
    fn a_launched_missile_that_misses_lands_where_its_flight_ends() {
        let mut rig = Rig::new();
        rig.app.insert_resource(crate::accuracy::HitRules(Box::new(Never)));
        rig.reach(5, 0);
        rig.launcher(0, true);
        let far = rig.mark(9);
        let missile = rig.missile(0);
        rig.throw(missile, 9);
        assert_eq!(rig.hp(far), 20);
        assert_eq!(rig.lying_at(rig.start.offset(9, 0)), vec![missile]);
    }

    #[test]
    fn a_launcher_launches_only_what_it_is_for_and_only_when_worn() {
        let mut rig = Rig::new();
        rig.reach(5, 0);
        rig.launcher(1, true);
        let far = rig.mark(9);
        let missile = rig.missile(0);
        rig.throw(missile, 9);
        assert_eq!(rig.hp(far), 20, "a launcher for something else adds nothing");

        let mut rig = Rig::new();
        rig.reach(5, 0);
        rig.launcher(0, false);
        let far = rig.mark(9);
        let missile = rig.missile(0);
        rig.throw(missile, 9);
        assert_eq!(rig.hp(far), 20, "nor one in the pack");
    }

    #[test]
    fn a_missed_throw_rests_where_a_hit_would_have_and_hurts_nobody() {
        let mut rig = Rig::new();
        rig.app.insert_resource(crate::accuracy::HitRules(Box::new(Never)));
        let target = rig.mark(3);
        let knife = rig.knives(1);
        let events = rig.throw(knife, 3);
        assert_eq!(rig.hp(target), 20, "a miss deals nothing");
        assert_eq!(rig.lying_at(rig.start.offset(3, 0)), vec![knife], "and lies where a hit would have left it");
        assert!(events.iter().any(|e| matches!(e, ItemEvent::Thrown { struck: None, .. })), "the item event says it struck nobody: {events:?}");
    }

    /// A worn thing thrown would come off, so a stuck one is not thrown:
    /// refused for nothing, said, and still in hand.
    #[test]
    fn a_stuck_thing_worn_is_not_thrown() {
        let mut rig = Rig::new();
        let target = rig.mark(3);
        let knife = rig.knives(1);
        let hand = atglyph_rules::EquipShape::in_slot(atglyph_rules::SlotId::from_raw(0));
        let mut worn = crate::items::Equipped(atglyph_rules::Equipment::with_slot_count(1));
        worn.equip(knife, &hand).expect("the slot exists");
        let player = rig.player;
        rig.app.world_mut().entity_mut(player).insert(worn);
        rig.app.world_mut().entity_mut(knife).insert((crate::items::Wearable(hand), crate::items::Stuck));
        assert_eq!(rig.throw(knife, 3), vec![ItemEvent::WouldNotComeOff { actor: player, item: knife }], "refused, and said");
        assert_eq!((rig.hp(target), rig.now()), (20, 0), "for nothing");
        assert!(rig.app.world().get::<crate::items::Equipped>(player).unwrap().contains(knife), "and still in hand");
        assert!(rig.app.world().get::<Inventory>(player).unwrap().contains(knife));
    }

    #[test]
    fn a_throw_that_strikes_a_body_in_between_is_rolled_against_that_body_at_its_distance() {
        let mut rig = Rig::new();
        rig.app.insert_resource(crate::accuracy::HitRules(Box::new(OnlyAt(2))));
        let near = rig.mark(2);
        let far = rig.mark(4);
        let knife = rig.knives(1);
        rig.throw(knife, 4);
        assert_eq!(rig.hp(near), 17, "rolled against the near mark at two cells, and it hit for the knife's three");
        assert_eq!(rig.hp(far), 20);
    }

    /// A grenade bursts where it comes down and is gone, not left lying
    /// there to be picked up again.
    #[test]
    fn a_thrown_consumable_lands_its_trigger_where_it_comes_down_and_is_gone() {
        let mut rig = Rig::new();
        let target = rig.mark(3);
        let grenade = rig.grenades(1);
        rig.throw(grenade, 3);
        rig.app.update();
        assert_eq!(rig.hp(target), 16, "the burst landed on whoever stood there");
        assert!(rig.app.world().get_entity(grenade).is_err(), "and the grenade is spent");
        let at = rig.start.offset(3, 0);
        assert!(rig.lying_at(at).is_empty(), "nothing lies where it burst");
    }

    /// Throwing one of a stack spends that one: the rest stay in the bag,
    /// one fewer, and not two.
    #[test]
    fn throwing_one_of_a_stack_spends_that_one_and_leaves_the_rest_in_the_bag() {
        let mut rig = Rig::new();
        let grenades = rig.grenades(3);
        rig.throw(grenades, 3);
        rig.app.update();
        assert_eq!(rig.app.world().get::<Stack>(grenades).map(|s| s.count), Some(2), "one thrown, two left");
        let at = rig.start.offset(3, 0);
        assert!(rig.lying_at(at).is_empty(), "and the one thrown is gone");
    }

    /// With something watching, a knife that has left the hand lies
    /// nowhere and hurts nobody until its flight has been seen.
    #[test]
    fn watched_a_thrown_knife_comes_down_only_after_its_flight_has_been_seen() {
        let mut rig = Rig::new();
        let knife = rig.knives(1);
        let near = rig.mark(2);
        rig.app.world_mut().resource_mut::<CueWait>().watch();

        let events = rig.throw(knife, 4);
        assert!(events.is_empty(), "nothing has landed");
        assert!(!rig.app.world().get::<Inventory>(rig.player).unwrap().contains(knife), "it left the hand");
        assert!(rig.app.world().get::<Position>(knife).is_none(), "and lies nowhere");
        assert_eq!(rig.hp(near), 20);
        assert_eq!(rig.now(), 0, "no turn is dealt while it flies, so the clock waits with it");
        assert!(rig.app.world().resource::<CueWait>().in_flight());

        rig.app.world_mut().resource_mut::<CueWait>().release();
        rig.app.update();
        let events: Vec<ItemEvent> = rig.app.world_mut().resource_mut::<Messages<ItemEvent>>().drain().collect();
        assert!(matches!(events.as_slice(), [ItemEvent::Thrown { struck: Some(who), .. }] if *who == near), "{events:?}");
        assert_eq!(rig.app.world().get::<Position>(knife).map(|p| p.0), Some(rig.start.offset(2, 0)), "at their feet");
        assert_eq!(rig.hp(near), 17, "and it hurt");
        assert!(!rig.app.world().resource::<CueWait>().in_flight());
        assert_eq!(rig.now(), 100, "and the turn the throw spent goes by");
    }

    #[test]
    fn a_thrown_knife_strikes_the_first_body_in_its_way_and_lies_at_its_feet() {
        let mut rig = Rig::new();
        let knives = rig.knives(3);
        let (near, far) = (rig.mark(2), rig.mark(4));

        let events = rig.throw(knives, 4);
        let [ItemEvent::Thrown { actor, item, at, struck, .. }] = events.as_slice() else { panic!("one throw: {events:?}") };
        assert_eq!((*actor, *at, *struck), (rig.player, Position(rig.start.offset(2, 0)), Some(near)), "the near one was in the way");
        assert_eq!((rig.hp(near), rig.hp(far)), (17, 20), "and took the knife");
        assert_ne!(*item, knives, "one left the stack as a thing of its own");
        assert_eq!(rig.app.world().get::<Stack>(*item).map(|s| s.count), Some(1));
        assert_eq!(rig.app.world().get::<Stack>(knives).map(|s| s.count), Some(2), "and two stay in hand");
        assert!(rig.app.world().get::<Inventory>(rig.player).unwrap().contains(knives));
        assert_eq!(rig.app.world().get::<Name>(*item).map(|n| n.as_str()), Some("knife"), "it is still a knife");
        assert_eq!(rig.now(), 100, "a throw is one action");
    }

    #[test]
    fn a_throw_falls_short_of_a_wall_and_lands_at_the_end_of_its_reach() {
        let mut rig = Rig::new();
        let knives = rig.knives(2);
        rig.app.world_mut().resource_mut::<WorldMap>().set_tile(rig.start.offset(3, 0), TileId(1));
        rig.app.update();
        let events = rig.throw(knives, 5);
        assert!(
            matches!(events.as_slice(), [ItemEvent::Thrown { at, struck: None, .. }] if *at == Position(rig.start.offset(2, 0))),
            "short of the wall: {events:?}"
        );

        rig.app.world_mut().resource_mut::<WorldMap>().set_tile(rig.start.offset(3, 0), TileId(2));
        rig.app.update();
        let last = rig.app.world().get::<Inventory>(rig.player).unwrap().items[0];
        let events = rig.throw(last, 7);
        assert!(
            matches!(events.as_slice(), [ItemEvent::Thrown { at, item, .. }] if *at == Position(rig.start.offset(5, 0)) && *item == last),
            "at its reach, and the last one is the stack itself: {events:?}"
        );
        assert!(rig.app.world().get::<Inventory>(rig.player).unwrap().items.is_empty(), "the hand is empty");
    }

    #[test]
    fn a_throw_of_nothing_carried_or_at_your_own_feet_is_refused_for_free() {
        let mut rig = Rig::new();
        let knives = rig.knives(2);
        assert!(rig.throw(knives, 0).is_empty(), "not at your own feet");
        let lying = rig.app.world_mut().spawn((Item, Position(rig.start.offset(1, 0)), Throwable::new(Some(3), None))).id();
        assert!(rig.throw(lying, 3).is_empty(), "not what lies on the ground");
        let stone = rig.app.world_mut().spawn(Item).id();
        rig.app.world_mut().get_mut::<Inventory>(rig.player).unwrap().items.push(stone);
        assert!(rig.throw(stone, 3).is_empty(), "not what was never made to be thrown");
        assert_eq!(rig.now(), 0);
    }

    #[test]
    fn what_the_dead_carried_falls_where_they_died() {
        let mut rig = Rig::new();
        let mark = rig.mark(2);
        let purse = rig.app.world_mut().spawn(Item).id();
        rig.app.world_mut().entity_mut(mark).insert(Inventory { items: vec![purse] });
        rig.app.world_mut().write_message(DamageEvent::new(mark, Hit::by(rig.player, rig.sides.kind, 99)));
        rig.app.world_mut().write_message(Intent::new(rig.player, crate::turn::Wait));
        rig.app.update();
        assert_eq!(rig.app.world().get::<Position>(purse).map(|p| p.0), Some(rig.start.offset(2, 0)), "it lies where its carrier fell");
        assert_eq!(rig.app.world().get::<OnMap>(purse).map(|m| m.0), Some(rig.app.world().resource::<WorldMap>().current()));
    }
}
