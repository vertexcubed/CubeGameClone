use std::f32::consts::PI;
use std::sync::Arc;
use bevy::prelude::{Component, IVec2};

pub trait HeightMapProvider {
    fn get_height(&self, pos: IVec2) -> i32;
}

#[derive(Component, Debug, Default)]
pub struct FlatHeightMap {
    height: i32
}
impl FlatHeightMap {
    pub fn new(height: i32) -> FlatHeightMap {
        FlatHeightMap { height }
    }
}
impl HeightMapProvider for FlatHeightMap {
    fn get_height(&self, _: IVec2) -> i32 {
        self.height
    }
}
#[derive(Component, Debug, Default)]
pub struct SineHeightMap {}
impl SineHeightMap {
    pub fn new() -> Self {
        Self {}
    }
}
impl HeightMapProvider for SineHeightMap {
    fn get_height(&self, pos: IVec2) -> i32 {
        (10.0 * f32::sin((2.0 * PI / 20.0) * (pos.x as f32))) as i32
    }
}


// all temporary lol
#[derive(Component, Debug)]
pub struct WorldGenerator<T: HeightMapProvider> {
    height_map: Arc<T>
}
impl <T: HeightMapProvider> WorldGenerator<T> {
    pub fn new(height_map: T) -> Self {
        Self {
            height_map: Arc::new(height_map)
        }
    }
    
    pub fn borrow_height_map(&self) -> Arc<T> {
        self.height_map.clone()
    }
}