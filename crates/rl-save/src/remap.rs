//! Stable ids for entities in a save.
//!
//! An `Entity` is a runtime handle: its index is reused and its
//! generation is meaningless in another process. A save refers to
//! entities by [`SaveId`], handed out densely as the save is captured, and
//! the same map binds them to fresh entities as the save is restored, so
//! a bag can list its items and a queue its actors across the trip.

use std::collections::HashMap;

use bevy::prelude::Entity;
use serde::{Deserialize, Serialize};

/// An entity's number in a save.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SaveId(pub u32);

/// Entities to save ids while capturing; save ids to entities while
/// restoring.
#[derive(Debug, Default)]
pub struct EntityRemap {
    to_save: HashMap<Entity, SaveId>,
    to_entity: Vec<Option<Entity>>,
}

impl EntityRemap {
    /// An empty map.
    pub fn new() -> Self {
        Self::default()
    }

    /// The save id of `entity`, allocated on first sight.
    pub fn save_id(&mut self, entity: Entity) -> SaveId {
        if let Some(id) = self.to_save.get(&entity) {
            return *id;
        }
        let id = SaveId(self.to_entity.len() as u32);
        self.to_save.insert(entity, id);
        self.to_entity.push(Some(entity));
        id
    }

    /// The save id of `entity`, if it was captured.
    pub fn saved(&self, entity: Entity) -> Option<SaveId> {
        self.to_save.get(&entity).copied()
    }

    /// Binds a save id to the entity restored for it.
    pub fn bind(&mut self, id: SaveId, entity: Entity) {
        let i = id.0 as usize;
        if self.to_entity.len() <= i {
            self.to_entity.resize(i + 1, None);
        }
        self.to_entity[i] = Some(entity);
        self.to_save.insert(entity, id);
    }

    /// The entity restored for `id`, if any.
    pub fn entity(&self, id: SaveId) -> Option<Entity> {
        self.to_entity.get(id.0 as usize).copied().flatten()
    }

    /// How many ids have been handed out or bound.
    pub fn len(&self) -> usize {
        self.to_entity.len()
    }

    /// Whether nothing was mapped.
    pub fn is_empty(&self) -> bool {
        self.to_entity.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_dense_stable_and_bind_back() {
        let (a, b) = (Entity::from_raw_u32(10).unwrap(), Entity::from_raw_u32(20).unwrap());
        let mut m = EntityRemap::new();
        assert_eq!(m.save_id(a), SaveId(0));
        assert_eq!(m.save_id(b), SaveId(1));
        assert_eq!(m.save_id(a), SaveId(0), "same entity, same id");
        assert_eq!(m.saved(b), Some(SaveId(1)));
        let mut r = EntityRemap::new();
        let c = Entity::from_raw_u32(30).unwrap();
        r.bind(SaveId(1), c);
        assert_eq!(r.entity(SaveId(1)), Some(c));
        assert_eq!(r.entity(SaveId(0)), None, "not restored yet");
        assert_eq!(r.saved(c), Some(SaveId(1)));
    }
}
