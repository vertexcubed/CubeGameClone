use std::collections::{HashMap, HashSet};
use bevy::math::IVec3;
use bevy::prelude::{Component, Entity};

/// A component that tracks machines in the world.
/// The machine world exists mostly separate from the block world, as machines should always be loaded.
#[derive(Component)]
pub struct MachineWorld {
    block_map: HashMap<IVec3, MachineBlock>,
    
    // TODO: is this necessary?
    machine_set: HashSet<Entity>
}
impl MachineWorld {
    pub fn new() -> Self {
        Self {
            block_map: HashMap::new(),
            machine_set: HashSet::new()
        }
    }
    
    pub fn get_machine(&self, pos: &IVec3) -> Option<&MachineBlock> {
        self.block_map.get(pos)
    }
}

#[derive(Debug, Clone)]
pub struct MachineBlock {
    pub entity: Entity,
    pub metadata: u8
}
impl MachineBlock {
    pub fn new(entity: Entity) -> Self {
        Self::with_metadata(entity, 0)
    }
    
    pub fn with_metadata(entity: Entity, metadata: u8) -> Self {
        Self {
            entity,
            metadata
        }
    }
}