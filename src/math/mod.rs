use bevy::prelude::{IVec3, Vec3};

pub mod ray;


pub trait Vec3Ext {
    fn as_block_pos(&self) -> IVec3;
}
impl Vec3Ext for Vec3 {
    fn as_block_pos(&self) -> IVec3 {
        self.floor().as_ivec3()
    }
}