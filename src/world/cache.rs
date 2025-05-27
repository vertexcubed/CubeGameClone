use bevy::prelude::{Commands, Component, Entity, IVec3};
use std::collections::HashMap;


/// Stores entity ids for all chunks currently loaded in the world / in memory.
#[derive(Component, Debug, Default)]
pub struct ChunkCache {
    map: HashMap<IVec3, Entity>
}

impl ChunkCache {
    // gets the chunk entity at this position. Cheap to clone.
    pub fn get_chunk(&self, pos: IVec3) -> Option<Entity> {
        if self.map.contains_key(&pos) {
            Some(self.map[&pos])
        }
        else {
            None
        }
    }
    
    pub fn add_to_cache(&mut self, pos: IVec3, id: Entity) {
        if self.map.contains_key(&pos) {
            panic!("Cannot add chunk to cache multiple times!");
        }
        else {
            self.map.insert(pos, id);
        }
    }
    
    pub fn remove_from_cache(&mut self, pos: IVec3, commands: &mut Commands) -> bool {
        match self.map.remove(&pos) {
            Some(e) => {
                commands.entity(e).despawn();
                true
            },
            None => false
        }
    }
}