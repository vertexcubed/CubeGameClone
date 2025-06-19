use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use bevy::asset::{AssetContainer, Assets, Handle};
use bevy::prelude::*;
use bimap::hash::Iter;
use bevy::log::info;
use bimap::BiMap;
use crate::asset::block::{BlockDef, BlockStateDef};
use crate::core::AllBlockDefs;
use crate::core::errors::RegistryError;


// container for all of a block's properties needed at runtime.
#[derive(Debug, Hash, PartialEq, Eq)]
pub struct Block {
    id: String,
    hardness: u32,
    default_state: BTreeMap<String, String>,
    states: Vec<BlockStateDef>
}

impl Block {


    pub fn get_id(&self) -> &String {
        &self.id
    }
    pub fn get_hardness(&self) -> u32 {
        self.hardness
    }
    pub fn get_default_state(&self) -> &BTreeMap<String, String> {
        &self.default_state
    }

    pub fn get_states(&self) -> &Vec<BlockStateDef> {
        &self.states
    }
}




#[derive(Resource, Clone, Debug)]
pub struct BlockRegistry {
    map: Arc<RwLock<BiMap<String, Arc<Block>>>>,
    
    frozen: bool,
}

impl BlockRegistry {

    pub fn new() -> Self {
        let mut map = BiMap::new();
        map.insert(String::from("air"), Arc::new(Block {
            id: String::from("air"),
            hardness: 0,
            default_state: BTreeMap::new(),
            states: vec![],
        }));
        BlockRegistry {
            map: Arc::new(RwLock::new(map)),
            frozen: false
        }
    }


    pub fn get_block(&self, name: &str) -> Option<Arc<Block>> {
        self.map.read().unwrap().get_by_left(name).cloned()
    }
    
    pub fn get_id(&self, block: &Block) -> Option<String> {
        self.map.read().unwrap().get_by_right(block).cloned()
    }
    
    pub fn add(&mut self, name: &str, value: Block) -> Result<(), RegistryError>{
        if self.frozen {
            return Err(RegistryError::Frozen("block".to_string()));
        }
        
        if let Some(_) = self.map.read().unwrap().get_by_left(name) {
            return Err(RegistryError::Duplicate(name.to_string(), "block".to_string()));
        }
        
        self.map.write().unwrap().insert(name.to_string(), Arc::new(value));
        Ok(())
    }
    
    pub fn borrow_map(&self) -> Arc<RwLock<BiMap<String, Arc<Block>>>> {
        self.map.clone()
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
    block_asset: Res<Assets<BlockDef>>,
    mut block_res: ResMut<BlockRegistry>,
    def_list: Res<AllBlockDefs>
) -> Result<(), RegistryError> {

    for h in def_list.inner.iter() {
        let b = block_asset.get(h).unwrap();
        let block = Block {
            id: b.id.clone(),
            hardness: b.hardness,
            default_state: b.default_state.clone(),
            states: b.states.clone()
        };
        block_res.add(b.id.as_str(), block)?;
    }
    block_res.freeze();

    Ok(())
}
