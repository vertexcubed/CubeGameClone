use bevy::math::{ivec3, IVec3, Vec3};

pub trait Vec3Ext {
    fn as_block_pos(&self) -> IVec3;
}

impl Vec3Ext for Vec3 {
    fn as_block_pos(&self) -> IVec3 {
        self.floor().as_ivec3()
    }
}

pub trait BlockPos {
    type VecType;
    fn center(&self) -> Vec3;
    
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
        self + ivec3(0, 0, 1)
    }

    fn west(&self) -> Self::VecType {
        self + ivec3(0, 0, -1)
    }
}