//! Shields: a worn [`Shield`] turns a blow, a shot or a throw aside whole,
//! at its chance, a few times a turn.
//!
//! [`Guarding`] is the one roll. The attack resolver asks it for each foe
//! a blow or a shot reaches, as the attack is made and before the hit
//! model's roll, and the throw resolver asks it for whoever a throw comes
//! down on, as it lands; a block lands nothing of the attack, which is
//! written as [`AttackBlocked`] where a hit would have been, and the attack still
//! spent its time and fired what firing spends. What a bearer has tried
//! this turn is kept in [`BlocksSpent`], filled again when it next holds the
//! turn, so the count stays the bearer's across a shield taken off and
//! another put on. [`Loadout::shield`](crate::combat::Loadout::shield) is
//! which shield a panel shows.
//!
//! Only a blow, a shot and a throw, the attacks that travel at someone, are
//! raised against: an effect, an ability, a fire, a trap gets through. A
//! bearer caught unaware of its attacker, by
//! [`Watchers::unaware`](crate::stealth::Watchers::unaware), has not raised
//! it, so a sleeper never blocks and neither does one a hider surprises.
//!
//! Its own plugin, [`ShieldsPlugin`], since a game with no shields has no
//! count to keep; without it nothing is ever blocked and a [`Shield`] is
//! only carried.

use atglyph_rules::Equipment;
use atglyph_rules::parts::ShieldDef;
use bevy::ecs::entity::Entities;
use bevy::prelude::*;
use std::collections::BTreeMap;

use crate::app::EngineApp;
use crate::combat::{CombatRng, DamageRoute};
use crate::components::MyTurn;
use crate::items::Equipped;

/// A shield, on an item: worn, it turns aside the attacks its bearer is
/// raised against at its [`ShieldDef`]'s chance.
///
/// No armor of its own: a shield and a helm are two answers to a blow, and
/// a game that wants both on one thing gives it an `Armor` too.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shield(pub ShieldDef);

/// An attack turned aside by a shield: nothing of it landed.
///
/// Written where a hit would have landed, as [`Missed`](crate::accuracy::Missed)
/// is: at once for a blow or an unseen shot, when the flight is seen for a
/// watched one, when the thing comes down for a throw. The weapon still
/// fired, so [`Struck`](crate::combat::Struck) and its `fire` moment were
/// written; no damage, no `hit` and no `struck` moment were. Stealth reads
/// it as it reads a miss, since the one it was aimed at knows where it came
/// from.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttackBlocked {
    /// Who attacked.
    pub attacker: Entity,
    /// Who turned it aside.
    pub target: Entity,
    /// The shield that turned it.
    pub shield: Entity,
    /// The worn or thrown item it was made with, if any.
    pub with: Option<Entity>,
    /// How it travelled: a blow, a shot or a throw.
    pub reach: DamageRoute,
}

/// How many blocks each bearer has tried since it last held the turn.
///
/// The bearer's rather than the shield's, so a shield swapped for another
/// in the middle of a round does not raise the count again: a round
/// shield's one block spent and a kite shield put on leaves one of the
/// kite's two. Emptied for an actor as it is dealt its turn, and of anyone
/// no longer in the world. A `BTreeMap` rather than a hash map, as every
/// map on a gameplay path is, though nothing here is iterated in an order
/// that matters.
#[derive(Resource, Default, Debug)]
pub struct BlocksSpent(BTreeMap<Entity, u32>);

impl BlocksSpent {
    /// The blocks `who` has tried since it last held the turn.
    pub fn spent(&self, who: Entity) -> u32 {
        self.0.get(&who).copied().unwrap_or(0)
    }
}

/// Gives every actor dealt a turn this pass its blocks back, and forgets
/// whoever has left the world.
///
/// In [`TurnSet::Decide`](crate::plugin::TurnSet::Decide), after the turn
/// is dealt and before anything resolves, so a blow struck at the bearer in
/// the same pass meets the shield raised again. A throw still in the air
/// is rolled for as it lands, so it meets the count standing then.
pub(crate) fn raise_shields(dealt: Query<Entity, Added<MyTurn>>, entities: &Entities, mut spent: ResMut<BlocksSpent>) {
    for who in &dealt {
        spent.0.remove(&who);
    }
    spent.0.retain(|who, _| entities.contains(*who));
}

/// The shield `worn` holds first, in slot order, with what it blocks.
pub(crate) fn worn_shield(worn: Option<&Equipment<Entity>>, shields: &Query<&Shield>) -> Option<(Entity, ShieldDef)> {
    worn?.worn().find_map(|(_, item)| shields.get(item).ok().map(|s| (item, s.0)))
}

/// The roll a shield makes, for a system that already holds what everyone
/// wears: the throw resolver, which takes things off.
///
/// [`Guarding`] is this with its own read of what is worn.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct Guard<'w, 's> {
    /// Absent without [`ShieldsPlugin`], and then nothing is blocked.
    spent: Option<ResMut<'w, BlocksSpent>>,
    shields: Query<'w, 's, &'static Shield>,
    watchers: crate::stealth::Watchers<'w, 's>,
}

impl Guard<'_, '_> {
    /// [`Guarding::block`], for a `defender` wearing `worn`.
    pub(crate) fn block_wearing(
        &mut self,
        defender: Entity,
        worn: Option<&Equipment<Entity>>,
        attacker: Entity,
        reach: DamageRoute,
        rng: &mut CombatRng,
    ) -> Option<Entity> {
        if !matches!(reach, DamageRoute::Melee | DamageRoute::Shot | DamageRoute::Thrown) {
            return None;
        }
        let spent = self.spent.as_deref_mut()?;
        let (shield, def) = worn_shield(worn, &self.shields)?;
        let tried = spent.spent(defender);
        if !def.raised(tried) || self.watchers.unaware(defender, attacker) {
            return None;
        }
        spent.0.insert(defender, tried + 1);
        def.turns_aside(&mut **rng).then_some(shield)
    }
}

/// Whether a defender's shield turns an attack aside.
///
/// The one roll, asked by every resolver that lands a blow, a shot or a
/// throw, so a panel's chance and the fight's agree.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Guarding<'w, 's> {
    guard: Guard<'w, 's>,
    worn: Query<'w, 's, &'static Equipped>,
}

impl Guarding<'_, '_> {
    /// Whether `defender`'s shield turns aside what `attacker` sent at it by
    /// `reach`, and if so which shield did.
    ///
    /// `None`, drawing nothing, for an attack that is not a blow, a shot or
    /// a throw, a defender with no shield worn, one whose blocks this turn
    /// are spent, and one unaware of its attacker. Otherwise one block is
    /// spent and one roll drawn, whether it turns the attack or not.
    pub fn block(&mut self, defender: Entity, attacker: Entity, reach: DamageRoute, rng: &mut CombatRng) -> Option<Entity> {
        let worn = self.worn.get(defender).ok().map(|w| &w.0);
        self.guard.block_wearing(defender, worn, attacker, reach, rng)
    }
}

/// Shields: the count of blocks each bearer has tried, filled again on its
/// turn, and [`AttackBlocked`].
///
/// Needs [`CombatPlugin`](crate::combat::CombatPlugin), whose resolvers
/// roll the block. Without this plugin a worn [`Shield`] blocks nothing and
/// the combat stream draws what it always did.
pub struct ShieldsPlugin;

impl Plugin for ShieldsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BlocksSpent>()
            .reset_on_new_run::<BlocksSpent>()
            .add_message::<AttackBlocked>()
            .add_systems(crate::plugin::Turn, raise_shields.in_set(crate::plugin::TurnSet::Decide));
    }

    fn finish(&self, app: &mut App) {
        app.depends_on::<crate::combat::CombatPlugin>("ShieldsPlugin");
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use atglyph_core::{DiceRoll, Point};
    use atglyph_rules::Brain;
    use atglyph_rules::ai::tactics::{Idle, MeleeAdjacent};
    use rand::Rng;

    use super::*;
    use crate::combat::{Attack, CombatPlugin, CombatRng, DamageEvent, DamageRoute, Faction, Health, MeleeAttack, RangedAttack, Struck};
    use crate::components::{Actor, Blocks, Player, Position, Viewshed};
    use crate::cue::CueWait;
    use crate::effects::{Fired, TriggerKindId, TriggerKinds};
    use crate::items::{Equipped, Inventory, Item, Wearable};
    use crate::minds::{Mind, MindsPlugin, Perception};
    use crate::state::EngineState;
    use crate::testing::TwoSides;
    use crate::throwing::{Throw, Throwable};
    use crate::turn::{Intent, Wait};

    /// What a test reads of what the turns wrote.
    #[derive(Resource, Default)]
    struct Heard {
        blocked: Vec<AttackBlocked>,
        struck: Vec<Struck>,
        damage: Vec<DamageEvent>,
        moments: Vec<(Entity, TriggerKindId)>,
    }

    fn hear(
        mut blocked: MessageReader<AttackBlocked>,
        mut struck: MessageReader<Struck>,
        mut damage: MessageReader<DamageEvent>,
        mut fired: MessageReader<Fired>,
        mut heard: ResMut<Heard>,
    ) {
        heard.blocked.extend(blocked.read().copied());
        heard.struck.extend(struck.read().copied());
        heard.damage.extend(damage.read().copied());
        heard.moments.extend(fired.read().map(|f| (f.on, f.moment)));
    }

    /// A player on open ground with a thousand health and a pack, in a game
    /// with combat, items, throwing, effects, shields and whatever `more`
    /// adds, play begun from the test's seed.
    struct Rig {
        app: App,
        start: Point,
        player: Entity,
        sides: TwoSides,
    }

    impl Rig {
        fn new(more: impl FnOnce(&mut App)) -> Rig {
            let mut app = crate::testing::headless_app();
            app.add_plugins((crate::fov::FovPlugin, CombatPlugin, crate::items::ItemsPlugin, crate::throwing::ThrowingPlugin, crate::effects::EffectsPlugin));
            app.add_plugins(ShieldsPlugin);
            crate::app::EngineApp::add_engine_effects(&mut app);
            more(&mut app);
            let start = crate::testing::ground(&mut app);
            let sides = crate::testing::two_sides(&mut app);
            app.init_resource::<Heard>().add_systems(PostUpdate, hear);
            let player = app
                .world_mut()
                .spawn((Actor, Player, Blocks, Position(start), Viewshed::new(8), Health::full(1000), Faction(sides.ours), Inventory::default()))
                .id();
            app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
            app.update();
            app.update();
            Rig { app, start, player, sides }
        }

        /// One of the player's foes `dx` east of it with a thousand health:
        /// an actor, mindless, so it waits out every turn it is dealt, or a
        /// post that is never dealt one and so never raises its shield again.
        fn foe(&mut self, dx: i32, actor: bool) -> Entity {
            let mut e = self.app.world_mut().spawn((Blocks, Position(self.start.offset(dx, 0)), Health::full(1000), Faction(self.sides.theirs)));
            if actor {
                e.insert(Actor);
            }
            let e = e.id();
            self.app.update();
            e
        }

        /// A goblin `dx` east and `dy` south of the player that strikes it
        /// for two whenever it stands beside it.
        fn goblin(&mut self, dx: i32, dy: i32) -> Entity {
            let brain = Arc::new(Brain::new().then(MeleeAdjacent).then(Idle { chance_pct: 0 }));
            let e = self
                .app
                .world_mut()
                .spawn((
                    (Actor, Blocks, Position(self.start.offset(dx, dy)), Health::full(1000), Faction(self.sides.theirs)),
                    (MeleeAttack::new(self.sides.kind, DiceRoll::flat(2)), Perception(8), Mind(brain)),
                ))
                .id();
            self.app.update();
            e
        }

        /// A shield blocking at `chance` percent, `blocks` times a turn.
        fn shield(&mut self, name: &str, chance: u32, blocks: u32) -> Entity {
            self.app.world_mut().spawn((Item, Shield(ShieldDef { chance, blocks }), Name::new(name.to_string()))).id()
        }

        /// `items` worn by `who`, each in a slot of its own in that order,
        /// in place of whatever it wore.
        fn wear(&mut self, who: Entity, items: &[Entity]) {
            let mut worn = atglyph_rules::Equipment::with_slot_count(items.len());
            for (slot, item) in items.iter().enumerate() {
                let shape = atglyph_rules::EquipShape::in_slot(atglyph_rules::SlotId::from_raw(slot as u32));
                self.app.world_mut().entity_mut(*item).insert(Wearable(shape.clone()));
                worn.equip(*item, &shape).expect("an empty slot");
            }
            self.app.world_mut().entity_mut(who).insert(Equipped(worn));
        }

        /// A sword striking for three, worn by the player.
        fn sword(&mut self) -> Entity {
            let sword = self.app.world_mut().spawn((Item, MeleeAttack::new(self.sides.kind, DiceRoll::flat(3)), Name::new("sword"))).id();
            self.wear(self.player, &[sword]);
            sword
        }

        /// A knife in the player's pack, struck for three, flying five.
        fn knife(&mut self) -> Entity {
            let knife = self.app.world_mut().spawn((Item, Throwable::new(Some(5), Some((self.sides.kind, DiceRoll::flat(3)))), Name::new("knife"))).id();
            self.app.world_mut().get_mut::<Inventory>(self.player).unwrap().items.push(knife);
            knife
        }

        fn attack(&mut self, target: Entity) {
            self.app.world_mut().write_message(Intent::new(self.player, Attack(target)));
            self.app.update();
        }

        fn throw(&mut self, item: Entity, at: Entity) {
            let at = self.app.world().get::<Position>(at).unwrap().0;
            self.app.world_mut().write_message(Intent::new(self.player, Throw { item, at }));
            self.app.update();
        }

        fn wait(&mut self) {
            self.app.world_mut().write_message(Intent::new(self.player, Wait));
            self.app.update();
        }

        /// What the turns wrote since the last time it was asked.
        fn heard(&mut self) -> Heard {
            std::mem::take(&mut *self.app.world_mut().resource_mut::<Heard>())
        }

        fn hp(&self, who: Entity) -> i32 {
            self.app.world().get::<Health>(who).unwrap().current
        }

        fn spent(&self, who: Entity) -> u32 {
            self.app.world().resource::<BlocksSpent>().spent(who)
        }

        /// The combat stream's next draw, which tells two worlds that have
        /// drawn the same from it apart from two that have not.
        fn next_draw(&mut self) -> u64 {
            self.app.world_mut().resource_mut::<CombatRng>().random()
        }
    }

    /// At a hundred percent a blow is turned aside whole: `AttackBlocked`, naming
    /// the shield and the sword, and no damage, no `hit` and no `struck`;
    /// at nothing it lands as any blow does.
    #[test]
    fn a_blow_on_a_shield_bearer_is_blocked_at_its_chance() {
        for (chance, blocks) in [(100, true), (0, false)] {
            let mut rig = Rig::new(|_| {});
            let sword = rig.sword();
            let bearer = rig.foe(1, false);
            let shield = rig.shield("round shield", chance, 1);
            rig.wear(bearer, &[shield]);
            rig.heard();
            rig.attack(bearer);
            let heard = rig.heard();
            let hit = heard.moments.contains(&(sword, TriggerKinds::HIT));
            let struck = heard.moments.contains(&(bearer, TriggerKinds::STRUCK));
            if blocks {
                let blocked = AttackBlocked { attacker: rig.player, target: bearer, shield, with: Some(sword), reach: DamageRoute::Melee };
                assert_eq!(heard.blocked, vec![blocked]);
                assert!(heard.damage.is_empty(), "nothing of the blow lands");
                assert!(!hit && !struck, "and it sets nothing off: {:?}", heard.moments);
                assert_eq!(rig.hp(bearer), 1000);
            } else {
                assert!(heard.blocked.is_empty(), "never blocked at nothing");
                assert_eq!(heard.damage.len(), 1);
                assert!(hit && struck, "the blow landed and set off what a blow does: {:?}", heard.moments);
                assert_eq!(rig.hp(bearer), 997);
            }
        }
    }

    /// A shot and a thrown knife are turned aside as a blow is, each named
    /// by how it came; the knife comes to rest at the bearer's feet.
    #[test]
    fn a_shot_and_a_throw_are_blocked() {
        let mut rig = Rig::new(|_| {});
        let player = rig.player;
        let kind = rig.sides.kind;
        rig.app.world_mut().entity_mut(player).insert(RangedAttack::new(kind, DiceRoll::flat(3), 6));
        let bearer = rig.foe(3, false);
        let shield = rig.shield("kite shield", 100, 2);
        rig.wear(bearer, &[shield]);
        rig.heard();
        rig.attack(bearer);
        assert_eq!(rig.heard().blocked, vec![AttackBlocked { attacker: player, target: bearer, shield, with: None, reach: DamageRoute::Shot }]);
        let knife = rig.knife();
        rig.throw(knife, bearer);
        assert_eq!(rig.heard().blocked, vec![AttackBlocked { attacker: player, target: bearer, shield, with: Some(knife), reach: DamageRoute::Thrown }]);
        assert_eq!(rig.hp(bearer), 1000, "neither landed");
        assert_eq!(rig.app.world().get::<Position>(knife).map(|p| p.0), Some(rig.start.offset(3, 0)), "the knife lies at the bearer's feet");
    }

    /// Damage that came as an effect is never a blow to raise a shield to:
    /// it lands whole, and the stream draws what a world with no shield
    /// draws.
    #[test]
    fn an_effect_is_never_blocked() {
        let draws = [true, false].map(|shielded| {
            let mut rig = Rig::new(|_| {});
            let bearer = rig.foe(1, false);
            if shielded {
                let shield = rig.shield("tower shield", 100, 3);
                rig.wear(bearer, &[shield]);
            }
            let hit = atglyph_rules::Hit::by(rig.player, rig.sides.kind, 5);
            rig.app.world_mut().write_message(DamageEvent::new(bearer, hit));
            rig.app.update();
            assert_eq!(rig.hp(bearer), 995, "shielded {shielded}: the whole of it landed");
            rig.next_draw()
        });
        assert_eq!(draws[0], draws[1], "and nothing was drawn for a block");
    }

    /// One block a turn: of two goblins striking in one round the first is
    /// turned aside and the second lands, and once the bearer has had its
    /// turn the first blow of the next round is turned aside again.
    #[test]
    fn a_turns_blocks_run_out_and_fill_again() {
        let mut rig = Rig::new(|app| {
            app.add_plugins(MindsPlugin);
        });
        let player = rig.player;
        let shield = rig.shield("round shield", 100, 1);
        rig.wear(player, &[shield]);
        rig.goblin(1, 0);
        rig.goblin(0, 1);
        rig.heard();
        for round in 0..3 {
            let before = rig.hp(player);
            rig.wait();
            let heard = rig.heard();
            let [first, second] = heard.struck.as_slice() else { panic!("round {round}: both goblins struck: {:?}", heard.struck) };
            assert_eq!(heard.blocked.iter().map(|b| b.attacker).collect::<Vec<_>>(), vec![first.attacker], "round {round}: the first is blocked");
            let landed: Vec<Option<Entity>> = heard.damage.iter().map(|d| d.hit.attacker).collect();
            assert_eq!(landed, vec![Some(second.attacker)], "round {round}: the second lands");
            assert_eq!(rig.hp(player), before - 2);
        }
    }

    /// What was spent is the bearer's, not the shield's: a round shield's
    /// one block spent and a kite shield put on in the same round leaves
    /// one block of the kite's two, not two.
    #[test]
    fn a_spent_block_stays_spent_across_a_shield_swap() {
        let mut rig = Rig::new(|_| {});
        rig.sword();
        let bearer = rig.foe(1, false);
        let (round, kite) = (rig.shield("round shield", 100, 1), rig.shield("kite shield", 100, 2));
        rig.wear(bearer, &[round]);
        rig.heard();
        rig.attack(bearer);
        assert_eq!(rig.heard().blocked.len(), 1, "the round shield's one block");
        rig.wear(bearer, &[kite]);
        rig.attack(bearer);
        assert_eq!(rig.heard().blocked.iter().map(|b| b.shield).collect::<Vec<_>>(), vec![kite], "one left of the kite's two");
        rig.attack(bearer);
        assert!(rig.heard().blocked.is_empty(), "and then none");
        assert_eq!(rig.hp(bearer), 997);
    }

    /// A throw is blocked from the count standing when it lands, not when
    /// it left the hand: watched, a knife thrown at a bearer whose one block
    /// was spent this round is turned aside if the bearer has its block
    /// back by the time it comes down, and lands if it has not.
    ///
    /// The engine deals nobody a turn while something flies, so the block
    /// is given back by hand while the knife is in the air, as the bearer's
    /// next turn would give it back; the bearer is a post, dealt no turn of
    /// its own, so nothing else gives it back.
    #[test]
    fn a_watched_throw_spends_the_block_standing_when_it_lands() {
        for given_back in [true, false] {
            let mut rig = Rig::new(|_| {});
            let bearer = rig.foe(2, false);
            let shield = rig.shield("round shield", 100, 1);
            rig.wear(bearer, &[shield]);
            rig.app.world_mut().resource_mut::<CueWait>().watch();
            rig.heard();
            let first = rig.knife();
            rig.throw(first, bearer);
            rig.app.world_mut().resource_mut::<CueWait>().release();
            rig.app.update();
            assert_eq!(rig.heard().blocked.len(), 1, "the first knife spent the block");
            let second = rig.knife();
            rig.throw(second, bearer);
            assert!(rig.app.world().resource::<CueWait>().in_flight(), "the second is in the air");
            assert_eq!(rig.spent(bearer), 1, "and the block is still spent as it leaves the hand");
            if given_back {
                rig.app.world_mut().resource_mut::<BlocksSpent>().0.remove(&bearer);
            }
            rig.app.world_mut().resource_mut::<CueWait>().release();
            rig.app.update();
            let blocked: Vec<Option<Entity>> = rig.heard().blocked.iter().map(|b| b.with).collect();
            if given_back {
                assert_eq!(blocked, vec![Some(second)], "it came down on a bearer with its block back, and was turned aside");
                assert_eq!(rig.hp(bearer), 1000);
            } else {
                assert!(blocked.is_empty(), "it came down on a bearer with its block spent: {blocked:?}");
                assert_eq!(rig.hp(bearer), 997);
            }
        }
    }

    /// A block tried is a block spent, whether the shield turned the blow
    /// or not.
    #[test]
    fn a_failed_block_still_spends_the_block() {
        let mut rig = Rig::new(|_| {});
        rig.sword();
        let bearer = rig.foe(1, false);
        let shield = rig.shield("round shield", 0, 1);
        rig.wear(bearer, &[shield]);
        rig.attack(bearer);
        assert_eq!(rig.hp(bearer), 997, "the blow landed");
        assert_eq!(rig.spent(bearer), 1, "and the block was spent on it");
    }

    /// A bearer that has not noticed a hidden attacker has not raised its
    /// shield: the blow lands and nothing is drawn for a block. Once it
    /// knows of the attacker it blocks.
    #[test]
    fn an_unaware_bearer_does_not_block() {
        let strike = |shielded: bool, knows: bool| {
            let mut rig = Rig::new(|app| {
                app.add_plugins((MindsPlugin, crate::stealth::StealthPlugin::per_observer()));
            });
            let player = rig.player;
            rig.app.world_mut().entity_mut(player).insert(crate::stealth::Stealth::default());
            rig.sword();
            let bearer = rig.foe(1, true);
            rig.app.world_mut().entity_mut(bearer).insert(crate::stealth::Notice::default());
            if shielded {
                let shield = rig.shield("round shield", 100, 1);
                rig.wear(bearer, &[shield]);
            }
            rig.app.update();
            let mut aware = rig.app.world_mut().get_mut::<crate::stealth::Aware>(bearer).unwrap();
            aware.0.clear();
            if knows {
                aware.0.insert(player, atglyph_rules::ai::Awareness::Alert { at: Point::ZERO, stale_turns: 0 });
            }
            rig.heard();
            rig.attack(bearer);
            (rig.heard().blocked.len(), 1000 - rig.hp(bearer), rig.next_draw())
        };
        let (blocked, took, draw) = strike(true, false);
        assert_eq!((blocked, took), (0, 3), "caught unaware, it took the blow");
        assert_eq!(draw, strike(false, false).2, "and drew what a bearer of no shield draws");
        assert_eq!(strike(true, true).0, 1, "knowing of its attacker, it blocks");
    }

    /// A world with no shield anywhere, in a game with shields: a player
    /// and a goblin beside it, each with a thousand health, trading ten
    /// blows from the test's seed. What each had left after each exchange.
    fn ten_exchanges() -> Vec<(i32, i32)> {
        let mut app = crate::testing::headless_app();
        app.add_plugins((crate::fov::FovPlugin, CombatPlugin, MindsPlugin, ShieldsPlugin));
        let start = crate::testing::ground(&mut app);
        let sides = crate::testing::two_sides(&mut app);
        let player = app
            .world_mut()
            .spawn((
                Actor,
                Player,
                Blocks,
                Position(start),
                Viewshed::new(8),
                Health::full(1000),
                Faction(sides.ours),
                MeleeAttack::new(sides.kind, DiceRoll::new(1, 6)),
            ))
            .id();
        let brain = Arc::new(Brain::new().then(MeleeAdjacent).then(Idle { chance_pct: 0 }));
        let goblin = app
            .world_mut()
            .spawn((
                (Actor, Blocks, Position(start.offset(1, 0)), Health::full(1000), Faction(sides.theirs)),
                (MeleeAttack::new(sides.kind, DiceRoll::new(1, 4)), Perception(8), Mind(brain)),
            ))
            .id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        let hp = |app: &App, who: Entity| app.world().get::<Health>(who).unwrap().current;
        (0..10)
            .map(|_| {
                app.world_mut().write_message(Intent::new(player, Attack(goblin)));
                app.update();
                (hp(&app, player), hp(&app, goblin))
            })
            .collect()
    }

    /// A tripwire: a world with no shield draws from the combat stream
    /// exactly what it drew before shields existed, so the same seed's ten
    /// exchanges leave the same health, copied from a run before the block.
    #[test]
    fn a_world_without_shields_draws_as_before() {
        assert_eq!(
            ten_exchanges(),
            vec![(999, 997), (998, 991), (995, 987), (992, 985), (989, 979), (986, 974), (983, 972), (982, 971), (981, 965), (977, 961)]
        );
    }
}
