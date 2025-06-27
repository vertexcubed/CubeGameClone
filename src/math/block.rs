use bevy::math::{ivec3, IVec3, Vec3};
use crate::world::block::Direction;

/// Extension trait for `Vec3`
pub trait Vec3Ext {
    fn as_block_pos(&self) -> IVec3;
}

impl Vec3Ext for Vec3 {
    fn as_block_pos(&self) -> IVec3 {
        self.floor().as_ivec3()
    }
}


/// Trait that represents a block pos in the world. In practice, this is just an extension trait for `IVec3`
pub trait BlockPos {
    type VecType;
    fn center(&self) -> Vec3;
    
    fn offset(&self, direction: Direction) -> Self::VecType;
    fn up(&self) -> Self::VecType;
    fn down(&self) -> Self::VecType;
    fn north(&self) -> Self::VecType;
    fn south(&self) -> Self::VecType;
    fn east(&self) -> Self::VecType;
    fn west(&self) -> Self::VecType;
}

impl BlockPos for IVec3 {
    type VecType = Self;

    fn center(&self) -> Vec3 {
        self.as_vec3() + 0.5
    }

    fn offset(&self, direction: Direction) -> Self::VecType {
        match direction {
            Direction::Up => self.up(),
            Direction::Down => self.down(),
            Direction::North => self.north(),
            Direction::South => self.south(),
            Direction::East => self.east(),
            Direction::West => self.west()
        }
    }

    fn up(&self) -> Self::VecType {
        self + ivec3(0, 1, 0)
    }

    fn down(&self) -> Self::VecType {
        self + ivec3(0, -1, 0)
    }

    fn north(&self) -> Self::VecType {
        self + ivec3(0, 0, 1)
    }

    fn south(&self) -> Self::VecType {
        self + ivec3(0, 0, -1)
    }

    fn east(&self) -> Self::VecType {
        self + ivec3(1, 0, 0)
    }

    fn west(&self) -> Self::VecType {
        self + ivec3(-1, 0, 0)
    }
}