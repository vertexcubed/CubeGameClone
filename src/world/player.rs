use bevy::prelude::Component;
use bevy::math::{IVec3, Vec3};
use crate::world::block::{BlockState, Direction};

#[derive(Component, Default)]
pub struct LookAtData {
    pub look_pos: Option<IVec3>,
    pub look_block: Option<BlockState>,
    pub surface: Option<Vec3>,
    pub face: Option<Direction>,
}

#[derive(Component, Default)]
pub struct BlockPicker {
    pub block_order: Vec<String>,
    pub index: usize,
}