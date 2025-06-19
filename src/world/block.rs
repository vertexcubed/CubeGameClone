use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use crate::registry::block::Block;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlockState {
    pub block: Arc<Block>,
    pub state: BTreeMap<String, String>
}
impl BlockState {
    pub fn new(block: Arc<Block>) -> Self {

        Self {
            state: block.get_default_state().clone(),
            block,
        }
    }
    
    pub fn with_state(block: Arc<Block>, state: BTreeMap<String, String>) -> Self {
        Self {
            block,
            state
        }
    }
    
    pub fn is_air(&self) -> bool {
        self.block.get_id() == "air"
    }
}