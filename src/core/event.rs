use crate::world::block::BlockState;
use bevy::prelude::{Event, IVec3, Vec3};


// TODO: migrate to Observer
#[derive(Event)]
pub struct PlayerMovedEvent {
    pub old: Vec3,
    pub new: Vec3
}


#[derive(Event)]
pub struct SetBlockEvent {
    pub pos: IVec3,
    pub old: BlockState,
    pub new: BlockState
}


#[derive(Event)]
pub struct JoinedWorldEvent {
    pub pos: Vec3,
}