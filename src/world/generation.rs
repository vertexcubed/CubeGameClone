use crate::math::NoiseFunction2D;
use crate::world::chunk;
use crate::world::chunk::ChunkData;
use bevy::prelude::{ivec2, ivec3, Component, IVec2};
use std::collections::HashMap;
use std::f32::consts::PI;
use std::sync::{Arc, OnceLock, RwLock};
use noiz::SampleableFor;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeightMapGroup([i32; HeightMapGroup::BLOCKS_PER_GROUP]);
impl HeightMapGroup {

    pub const BLOCKS_PER_GROUP: usize = ChunkData::CHUNK_SIZE * ChunkData::CHUNK_SIZE;

    pub fn new(data: [i32; HeightMapGroup::BLOCKS_PER_GROUP]) -> Self {
        Self(data)
    }

    pub fn get(&self, local_pos: IVec2) -> i32 {
        self.get_index(Self::delinearize(local_pos))
    }

    pub fn get_index(&self, index: usize) -> i32 {
        self.0[index]
    }

    pub fn delinearize(local_pos: IVec2) -> usize {
        ChunkData::CHUNK_SIZE * local_pos.y as usize + local_pos.x as usize
    }
}


pub trait HeightMapProvider: Send + Sync {

    /// Gets the height of this heightmap at the position pos. This is a (x, z) position in the world.
    /// If getting multiple values is required, it's recommended to use [`HeightMapProvider::get_chunk`] instead.
    fn get_height(&self, pos: IVec2) -> i32;
    /// Gets the height of this heightmap at all positions in this chunk.
    fn get_chunk(&self, chunk_pos: IVec2) -> HeightMapGroup;
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

    fn get_chunk(&self, _: IVec2) -> HeightMapGroup {
        HeightMapGroup::new([self.height; HeightMapGroup::BLOCKS_PER_GROUP])
    }
}
#[derive(Component, Debug, Default)]
pub struct SineHeightMap {}
impl SineHeightMap {
    pub fn new() -> Self {
        Self {}
    }
}
impl SineHeightMap {
    fn sine_func(x: f32) -> f32 {
        10.0 * f32::sin((2.0 * PI / 20.0) * x)
    }
}
impl HeightMapProvider for SineHeightMap {
    fn get_height(&self, pos: IVec2) -> i32 {
        SineHeightMap::sine_func(pos.x as f32) as i32
    }

    fn get_chunk(&self, chunk_pos: IVec2) -> HeightMapGroup {

        let mut out = [0; HeightMapGroup::BLOCKS_PER_GROUP];
        for y in 0..ChunkData::CHUNK_SIZE {
            for x in 0..ChunkData::CHUNK_SIZE {
                out[HeightMapGroup::delinearize(ivec2(x as i32, y as i32))] =
                    SineHeightMap::sine_func(
                        (chunk_pos.x as usize * ChunkData::CHUNK_SIZE + x) as f32
                    ) as i32;
            }
        }
        HeightMapGroup::new(out)
    }
}


// all temporary lol
#[derive(Component)]
pub struct WorldGenerator {
    height_map: Arc<dyn HeightMapProvider>
}
impl WorldGenerator {
    pub fn new(height_map: impl HeightMapProvider + 'static) -> Self {
        Self {
            height_map: Arc::new(height_map)
        }
    }
    
    pub fn borrow_height_map(&self) -> Arc<dyn HeightMapProvider> {
        self.height_map.clone()
    }
}


// one downside of Noiz: ts is type hell
pub struct NoiseHeightMap<N: NoiseFunction2D> {
    generator: noiz::Noise<N>,
    map: RwLock<HashMap<IVec2, Arc<OnceLock<HeightMapGroup>>>>
}
impl <N: NoiseFunction2D> NoiseHeightMap<N> {
    pub fn new(generator: noiz::Noise<N>) -> Self {
        NoiseHeightMap {
            generator,
            map: RwLock::new(HashMap::new())
        }
    }
    fn create_noise(&self, chunk_pos: IVec2) -> HeightMapGroup {
        let mut out = [0; HeightMapGroup::BLOCKS_PER_GROUP];
        for y in 0..ChunkData::CHUNK_SIZE {
            for x in 0..ChunkData::CHUNK_SIZE {

                let point = (chunk_pos * ChunkData::CHUNK_SIZE as i32) + ivec2(x as i32, y as i32);

                let noise_value: f32 = (self.generator.sample(point.as_vec2()));

                // let noise_value = point.y;



                out[HeightMapGroup::delinearize(ivec2(x as i32, y as i32))] = noise_value as i32;
            }
        }

        HeightMapGroup::new(out)
    }
}
impl <N: NoiseFunction2D + Send + Sync> HeightMapProvider for NoiseHeightMap<N> {

    // surprisingly not unsafe!
    fn get_height(&self, pos: IVec2) -> i32 {
        let chunk_pos = chunk::pos_to_chunk_pos(ivec3(pos.x, 0, pos.y));
        let chunk_pos = ivec2(chunk_pos.x, chunk_pos.z);
        let chunk_local = chunk::pos_to_chunk_local(ivec3(pos.x, 0, pos.y));
        let chunk_local = ivec2(chunk_local.x, chunk_local.z);

        self.get_chunk(chunk_pos).get(chunk_local)
    }

    fn get_chunk(&self, chunk_pos: IVec2) -> HeightMapGroup {
        // read from the map first
        let read = self.map.read().unwrap();
        let data_ref = read.get(&chunk_pos).cloned();
        // drop the read before we start writing! Or else a deadlock will likely occur
        drop(read);

        // If the data doesn't exist, then we need to write to the hashmap
        let data_ref = if data_ref.is_none() {
            // we write no data to prevent expensive call from slowing down this thread
            let mut write = self.map.write().unwrap();
            let ret = Arc::new(OnceLock::new());
            write.insert(chunk_pos, ret.clone());
            ret
        } else { data_ref.unwrap() };

        // get or init. Shouldn't cause race conditions since its the same init function always
        // TODO: this clone might be bad. Maybe find a way to not do this.
        data_ref.get_or_init(|| {self.create_noise(chunk_pos)}).clone()
    }
}