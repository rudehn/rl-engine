//! Items: on the ground, in a bag, or worn.
//!
//! An item is an entity. On the ground it has a [`Position`]; in a bag it
//! is listed in the carrier's [`Inventory`] and has none; worn, it is also
//! claimed in the carrier's [`Equipped`] slots. The engine resolves the
//! moves between those three states and charges a turn for each. What an
//! item does when used is the game's: the engine reports
//! [`ItemEvent::Used`] and the game reads it, applies the effect, and
//! despawns the item if it was consumed.

use bevy::prelude::*;
use rl_core::turn::BASE_ACTION_COST;
use rl_rules::{EquipShape, Equipment};

use crate::components::{MyTurn, Position};
use crate::turn::{Action, ActionDone, ActionRefused, Intent};

/// An item.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Item;

/// What an actor carries, in pickup order. Worn items stay listed here.
#[derive(Component, Debug, Clone, Default)]
pub struct Inventory {
    /// The items.
    pub items: Vec<Entity>,
}

impl Inventory {
    /// Whether `item` is carried.
    pub fn contains(&self, item: Entity) -> bool {
        self.items.contains(&item)
    }

    /// Drops `item` from the list. Returns whether it was there.
    pub fn remove(&mut self, item: Entity) -> bool {
        let before = self.items.len();
        self.items.retain(|i| *i != item);
        before != self.items.len()
    }
}

/// What an actor wears.
#[derive(Component, Debug, Clone, Deref, DerefMut)]
pub struct Equipped(pub Equipment<Entity>);

/// Where an item goes when worn. Items without this cannot be equipped.
#[derive(Component, Debug, Clone)]
pub struct Wearable(pub EquipShape);

/// A countable item: picking one up merges it into a carried item with the
/// same key instead of adding an entry. The key is the game's, usually
/// the definition id.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stack {
    /// What it is a stack of.
    pub key: u64,
    /// How many.
    pub count: u32,
}

/// What happened to an item, for narration and for the game's own
/// reactions. Written in [`TurnSet::Resolve`](crate::plugin::TurnSet::Resolve).
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemEvent {
    /// `actor` took `item` off the ground. `merged_into` names the carried
    /// stack it joined, in which case `item` no longer exists.
    PickedUp {
        /// Who.
        actor: Entity,
        /// What.
        item: Entity,
        /// The stack it was merged into, if any.
        merged_into: Option<Entity>,
    },
    /// `actor` put `item` on the ground at `at`.
    Dropped {
        /// Who.
        actor: Entity,
        /// What.
        item: Entity,
        /// Where.
        at: Position,
    },
    /// `actor` put `item` on.
    Equipped {
        /// Who.
        actor: Entity,
        /// What.
        item: Entity,
    },
    /// `actor` took `item` off, whether by choice or because something
    /// else took its slot.
    Unequipped {
        /// Who.
        actor: Entity,
        /// What.
        item: Entity,
    },
    /// `actor` used `item`. The game decides what that means.
    Used {
        /// Who.
        actor: Entity,
        /// What.
        item: Entity,
    },
}

/// The actor holding the turn, as the item resolver sees it.
type Carrier<'w, 's> = Query<'w, 's, (&'static Position, &'static mut Inventory, Option<&'static mut Equipped>), With<MyTurn>>;

/// Items on the ground.
type Ground<'w, 's> = Query<'w, 's, (Entity, &'static Position), With<Item>>;

/// What the item resolver reads and moves.
#[derive(bevy::ecs::system::SystemParam)]
pub struct ItemWorld<'w, 's> {
    carriers: Carrier<'w, 's>,
    ground: Ground<'w, 's>,
    stacks: Query<'w, 's, &'static Stack>,
    wearables: Query<'w, 's, &'static Wearable>,
    players: Query<'w, 's, (), With<crate::components::Player>>,
}

/// What the item resolver reports.
#[derive(bevy::ecs::system::SystemParam)]
pub struct ItemReport<'w> {
    done: MessageWriter<'w, ActionDone>,
    refused: MessageWriter<'w, ActionRefused>,
    events: MessageWriter<'w, ItemEvent>,
}

/// Resolves pickups, drops, equips, unequips and uses for the actor
/// holding the turn. Each costs one action. An impossible one, such as
/// picking up from bare ground, is refused for the player and treated as
/// a wait for anyone else, like an impossible move.
pub fn resolve_items(mut commands: Commands, mut intents: MessageReader<Intent>, world: ItemWorld, report: ItemReport) {
    let ItemWorld { mut carriers, ground, stacks, wearables, players } = world;
    let ItemReport { mut done, mut refused, mut events } = report;
    let mut acted: Vec<Entity> = Vec::new();
    for intent in intents.read() {
        let actor = intent.actor;
        if acted.contains(&actor) {
            continue;
        }
        let ok = match intent.action {
            Action::PickUp => {
                let Ok((pos, mut bag, _)) = carriers.get_mut(actor) else { continue };
                let here: Vec<Entity> = ground.iter().filter(|(_, p)| p.0 == pos.0).map(|(e, _)| e).collect();
                for item in &here {
                    commands.entity(*item).remove::<Position>();
                    let merged_into = stacks.get(*item).ok().and_then(|s| {
                        bag.items.iter().copied().find(|c| stacks.get(*c).is_ok_and(|t| t.key == s.key))
                    });
                    match merged_into {
                        Some(into) => {
                            let add = stacks.get(*item).map(|s| s.count).unwrap_or(1);
                            commands.entity(into).entry::<Stack>().and_modify(move |mut s| s.count += add);
                            commands.entity(*item).despawn();
                        }
                        None => bag.items.push(*item),
                    }
                    events.write(ItemEvent::PickedUp { actor, item: *item, merged_into });
                }
                !here.is_empty()
            }
            Action::Drop(item) => {
                let Ok((pos, mut bag, worn)) = carriers.get_mut(actor) else { continue };
                if !bag.remove(item) {
                    false
                } else {
                    if let Some(mut worn) = worn
                        && worn.unequip(item)
                    {
                        events.write(ItemEvent::Unequipped { actor, item });
                    }
                    commands.entity(item).insert(*pos);
                    events.write(ItemEvent::Dropped { actor, item, at: *pos });
                    true
                }
            }
            Action::Equip(item) => {
                let Ok((_, bag, worn)) = carriers.get_mut(actor) else { continue };
                match (worn, wearables.get(item)) {
                    (Some(mut worn), Ok(shape)) if bag.contains(item) => match worn.equip(item, &shape.0) {
                        Ok(displaced) => {
                            for other in displaced {
                                events.write(ItemEvent::Unequipped { actor, item: other });
                            }
                            events.write(ItemEvent::Equipped { actor, item });
                            true
                        }
                        Err(e) => {
                            warn!("{actor:?} could not equip {item:?}: {e}");
                            false
                        }
                    },
                    _ => false,
                }
            }
            Action::Unequip(item) => {
                let Ok((_, _, worn)) = carriers.get_mut(actor) else { continue };
                let taken_off = worn.is_some_and(|mut worn| worn.unequip(item));
                if taken_off {
                    events.write(ItemEvent::Unequipped { actor, item });
                }
                taken_off
            }
            Action::Use(item) => {
                let Ok((_, bag, _)) = carriers.get_mut(actor) else { continue };
                if bag.contains(item) {
                    events.write(ItemEvent::Used { actor, item });
                    true
                } else {
                    false
                }
            }
            _ => continue,
        };
        acted.push(actor);
        if ok || players.get(actor).is_err() {
            done.write(ActionDone { actor, cost: BASE_ACTION_COST });
        } else {
            refused.write(ActionRefused { actor });
        }
    }
}

/// Drops despawned items from every bag and every slot.
pub fn forget_removed_items(mut removed: RemovedComponents<Item>, mut carriers: Query<(&mut Inventory, Option<&mut Equipped>)>) {
    let gone: Vec<Entity> = removed.read().collect();
    if gone.is_empty() {
        return;
    }
    for (mut bag, worn) in &mut carriers {
        bag.items.retain(|i| !gone.contains(i));
        if let Some(mut worn) = worn {
            for item in &gone {
                worn.unequip(*item);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{Actor, Blocks, Player, RevealsMap, Speed, Viewshed};
    use crate::knowledge::Knowledge;
    use crate::plugin::headless_app;
    use crate::state::EngineState;
    use crate::turn::Turns;
    use crate::world::{ChunkRulesRes, WorldMap, WorldRes};
    use rl_content::Registry;
    use rl_core::{Point, RunSeed};
    use rl_grid::{TileId, TileRegistry};
    use rl_mapgen::Chain;
    use rl_mapgen::passes::Fill;
    use rl_rules::SlotDef;
    use rl_world::{BandId, CellFacts, ChunkContext, ChunkRules, Layers, Site, Surroundings, WorldConfig, WorldGraph, WorldRules};

    struct Flat;
    impl WorldRules for Flat {
        fn classify(&self, f: &CellFacts) -> BandId {
            BandId(if f.is_sea { 0 } else { 1 })
        }
        fn road_friction(&self, _: BandId, _: &CellFacts) -> Option<f32> {
            None
        }
        fn settlements(&self, _: &Layers, _: u64) -> Vec<Site> {
            Vec::new()
        }
    }
    struct Open(TileRegistry);
    impl ChunkRules for Open {
        fn tiles(&self) -> &TileRegistry {
            &self.0
        }
        fn fill(&self, _: &Surroundings) -> TileId {
            self.0.expect("floor")
        }
        fn chain(&self, _: &WorldGraph, _: &Surroundings) -> Chain<ChunkContext> {
            Chain::new().then(Fill { tile: self.0.expect("floor") })
        }
    }

    struct Rig {
        app: App,
        player: Entity,
        start: Point,
        main: rl_rules::SlotId,
        off: rl_rules::SlotId,
    }

    fn rig() -> Rig {
        let mut app = headless_app();
        let tiles = TileRegistry::standard();
        let world = WorldGraph::generate(RunSeed(5), WorldConfig { region_size: 16, ..WorldConfig::regions(12, 10) }, &Flat);
        let (region, _) = world.layers().bands.iter().find(|(_, b)| b.0 == 1).expect("land");
        let start = world.tile_origin(region).offset(8, 8);
        app.insert_resource(WorldMap::new(16, tiles.tables()));
        app.insert_resource(WorldRes(world));
        app.insert_resource(ChunkRulesRes(Box::new(Open(tiles))));
        app.insert_resource(Knowledge::new(16));
        let slots = Registry::from_defs(vec![SlotDef::new("main"), SlotDef::new("off")]).unwrap();
        let (main, off) = (slots.expect("main"), slots.expect("off"));
        let player = app
            .world_mut()
            .spawn((Actor, Player, Blocks, Position(start), Viewshed::new(6), RevealsMap, Speed(100), Inventory::default(), Equipped(Equipment::for_slots(&slots))))
            .id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        assert!(app.world().get::<MyTurn>(player).is_some());
        Rig { app, player, start, main, off }
    }

    fn act(rig: &mut Rig, action: Action) -> Vec<ItemEvent> {
        rig.app.world_mut().write_message(Intent { actor: rig.player, action });
        rig.app.update();
        let mut events = rig.app.world_mut().resource_mut::<Messages<ItemEvent>>();
        events.drain().collect()
    }

    #[test]
    fn pick_up_carry_wear_and_drop() {
        let mut r = rig();
        let sword = r.app.world_mut().spawn((Item, Position(r.start), Wearable(EquipShape::in_slot(r.main)))).id();
        let axe = r.app.world_mut().spawn((Item, Position(r.start), Wearable(EquipShape::in_slot(r.main).and_claims(r.off)))).id();
        let events = act(&mut r, Action::PickUp);
        assert_eq!(events.len(), 2);
        assert!(r.app.world().get::<Position>(sword).is_none(), "off the ground");
        assert_eq!(r.app.world().get::<Inventory>(r.player).unwrap().items, vec![sword, axe]);
        assert_eq!(r.app.world().resource::<Turns>().now(), 100, "picking up cost a turn");

        assert_eq!(act(&mut r, Action::Equip(sword)), vec![ItemEvent::Equipped { actor: r.player, item: sword }]);
        assert_eq!(
            act(&mut r, Action::Equip(axe)),
            vec![ItemEvent::Unequipped { actor: r.player, item: sword }, ItemEvent::Equipped { actor: r.player, item: axe }],
            "the two-hander pushed the sword out"
        );
        let worn = r.app.world().get::<Equipped>(r.player).unwrap();
        assert_eq!(worn.in_slot(r.off), Some(axe));
        assert!(r.app.world().get::<Inventory>(r.player).unwrap().contains(sword), "still carried");

        let events = act(&mut r, Action::Drop(axe));
        assert_eq!(events, vec![ItemEvent::Unequipped { actor: r.player, item: axe }, ItemEvent::Dropped { actor: r.player, item: axe, at: Position(r.start) }]);
        assert_eq!(r.app.world().get::<Position>(axe).unwrap().0, r.start);
        assert!(!r.app.world().get::<Inventory>(r.player).unwrap().contains(axe));
        assert!(r.app.world().get::<Equipped>(r.player).unwrap().is_free(r.main));
    }

    #[test]
    fn stacks_merge_and_impossible_actions_are_refused_for_free() {
        let mut r = rig();
        let coins = r.app.world_mut().spawn((Item, Position(r.start), Stack { key: 7, count: 5 })).id();
        act(&mut r, Action::PickUp);
        let more = r.app.world_mut().spawn((Item, Position(r.start), Stack { key: 7, count: 3 })).id();
        let events = act(&mut r, Action::PickUp);
        assert_eq!(events, vec![ItemEvent::PickedUp { actor: r.player, item: more, merged_into: Some(coins) }]);
        assert_eq!(r.app.world().get::<Stack>(coins).unwrap().count, 8);
        assert!(r.app.world().get_entity(more).is_err(), "the merged stack is gone");
        assert_eq!(r.app.world().get::<Inventory>(r.player).unwrap().items, vec![coins]);

        let before = r.app.world().resource::<Turns>().now();
        assert!(act(&mut r, Action::PickUp).is_empty(), "nothing here");
        assert!(act(&mut r, Action::Equip(coins)).is_empty(), "not wearable");
        assert_eq!(r.app.world().resource::<Turns>().now(), before, "refusals cost nothing");
        assert!(r.app.world().get::<MyTurn>(r.player).is_some());

        assert_eq!(act(&mut r, Action::Use(coins)), vec![ItemEvent::Used { actor: r.player, item: coins }]);
        assert_eq!(r.app.world().resource::<Turns>().now(), before + 100, "using is the game's, the turn is ours");
    }

    #[test]
    fn a_despawned_item_leaves_the_bag_and_the_slots() {
        let mut r = rig();
        let sword = r.app.world_mut().spawn((Item, Position(r.start), Wearable(EquipShape::in_slot(r.main)))).id();
        act(&mut r, Action::PickUp);
        act(&mut r, Action::Equip(sword));
        r.app.world_mut().entity_mut(sword).despawn();
        r.app.update();
        assert!(r.app.world().get::<Inventory>(r.player).unwrap().items.is_empty());
        assert!(r.app.world().get::<Equipped>(r.player).unwrap().is_free(r.main));
    }
}
