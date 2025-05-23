use bevy::asset::{Assets, Handle};
use bevy::prelude::*;
use bimap::hash::Iter;
use bevy::log::info;
use bimap::BiMap;
use crate::asset::block::Block;
use crate::core::errors::RegistryError;


#[derive(Resource, Default, Debug)]
pub struct BlockRegistry {
    map: BiMap<String, Handle<Block>>,
    
    frozen: bool,
}

impl BlockRegistry {
    pub fn get_block(&self, name: &str) -> Option<Handle<Block>> {
        self.map.get_by_left(name).cloned()
    }
    
    pub fn get_id(&self, block: &Handle<Block>) -> Option<String> {
        self.map.get_by_right(block).cloned()
    }
    
    pub fn add(&mut self, name: &str, value: Handle<Block>) -> Result<(), RegistryError>{
        if self.frozen {
            return Err(RegistryError::Frozen("block".to_string()));
        }
        
        if let Some(_) = self.map.get_by_left(name) {
            return Err(RegistryError::Duplicate(name.to_string(), "block".to_string()));
        }
        
        self.map.insert(name.to_string(), value);
        Ok(())
    }
    
    pub fn iter(&self) -> Iter<'_, String, Handle<Block>> {
        self.map.iter()
    }
    
    pub fn is_frozen(&self) -> bool {
        self.frozen
    }
    
    // purposely cannot unfreeze a registry
    pub fn freeze(&mut self) {
        self.frozen = true;
        info!("Freezing block registry.");
    }
}





pub fn load_blocks(
    block_asset: Res<Assets<Block>>,
    mut block_res: ResMut<BlockRegistry>,
    block_vec: Vec<Handle<Block>>
) -> Result<(), RegistryError> {

    for h in block_vec {
        let b = block_asset.get(&h).unwrap();
        block_res.add(b.id.as_str(), h)?;
    }
    block_res.freeze();

    Ok(())
}
