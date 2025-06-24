use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use bevy::prelude::error;
use serde::{Deserialize, Serialize};
use crate::core::errors::BlockStateError;
use crate::core::errors::BlockStateError::InvalidId;
use crate::registry::block::Block;
use crate::registry::Registry;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlockState {
    block: String,
    state: BTreeMap<String, String>
}
impl BlockState {
    pub fn new(id: &str, block_reg: &Registry<Block>) -> Result<Self, BlockStateError> {
        match block_reg.get(id) {
            Some(block) => {
                Ok(Self {
                    state: block.get_default_state().clone(),
                    block: String::from(id),
                })
            }
            None => {
                Err(InvalidId(String::from(id)))
            }
        }
    }
    
    pub fn with_state(
        id: &str, 
        state: BTreeMap<String, String>, 
        block_reg: &Registry<Block>) -> Result<Self, BlockStateError> {
        match block_reg.get(id) {
            Some(_) => {
                Ok(Self {
                    state,
                    block: String::from(id),
                })
            }
            None => {
                Err(InvalidId(String::from(id)))
            }
        }
    }
    
    pub fn get_id(&self) -> &str {
        self.block.as_str()
    }
    
    pub fn get_state(&self) -> &BTreeMap<String, String> {
        &self.state
    }

    pub fn is_air(&self) -> bool {
        self.block == "air"
    }
}
#[derive(Debug, Copy, Clone, PartialEq, Deserialize, Serialize)]
pub enum Direction {
    Up,
    Down,
    North,
    South,
    East,
    West
}