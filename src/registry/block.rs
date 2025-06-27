use crate::asset::block::{BlockAsset, BlockStateAsset};
use crate::registry::RegistryObject;
use bevy::prelude::*;
use std::collections::BTreeMap;

/// The registry representation of a Block. Contains its id, state information, etc.
#[derive(Debug, Hash, PartialEq, Eq)]
pub struct Block {
    id: String,
    hardness: u32,
    default_state: BTreeMap<String, String>,
    states: Vec<BlockStateAsset>
}

impl Block {
    
    /// Creates a Block from a corresponding BlockAsset.
    pub fn from_asset(asset: &BlockAsset) -> Block {
        Block {
            id: asset.id.clone(),
            hardness: asset.hardness,
            default_state: asset.default_state.clone(),
            states: asset.states.clone()
        }
    }
    pub fn get_hardness(&self) -> u32 {
        self.hardness
    }
    pub fn get_default_state(&self) -> &BTreeMap<String, String> {
        &self.default_state
    }

    pub fn get_states(&self) -> &Vec<BlockStateAsset> {
        &self.states
    }
}
impl RegistryObject for Block {
    fn get_id(&self) -> &str {
        self.id.as_str()
    }

    fn make_initial() -> Option<Self> {
        Some(Self {
            id: String::from("air"),
            hardness: 0,
            default_state: BTreeMap::new(),
            states: vec![],
        })
    }
}