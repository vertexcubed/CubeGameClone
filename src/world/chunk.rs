use std::slice::Iter;
use std::sync::{Arc, RwLock};
use bevy::log::info_span;
use crate::core::errors::ChunkError;
use crate::math::block::Vec3Ext;
use crate::world::block::BlockState;
use bevy::math::ivec3;
use bevy::prelude::{Component, Entity, IVec3, Transform};
use bitvec::bitvec;
use bitvec::order::Msb0;
use bitvec::prelude::BitVec;
use bitvec::view::BitViewSized;
use crate::math;

/// A data structure that represents a chunk in the world. Stores some information about it tied to
/// its physical state, like the blocks in the chunk and its state.
#[derive(Debug, Clone)]
pub struct Chunk {
    // the position of this chunk in the world. Should always be the same.
    pos: IVec3,
    // data may be read by multiple threads, but only modified by one thread.
    // Structure somewhat mirrors how chunks are stored to disk
    // Note: this may not be available! Especially if the chunk is not generated yet.
    data: Option<Arc<RwLock<ChunkData>>>,
    // The entity ID of the corresponding entity.
    // The entity stores all mesh information and rendering data and other in world data
    chunk_entity: Entity,
    generation_status: ChunkGenerationStatus,
}

impl Chunk {
    pub fn new(pos: IVec3, chunk_entity: Entity) -> Self {
        Self {
            pos,
            data: None,
            chunk_entity,
            generation_status: ChunkGenerationStatus::NotGenerated
        }
    }

    /// borrows the inner ChunkData. Mostly used for meshing on other threads, or for bulk read/writes / specific operations on the data.
    /// Unlike the main getter/setter method, you CAN read and write while a chunk is not fully generated, however this method still returns
    /// an error if the chunk data is None.
    pub fn get_data(&self) -> Result<Arc<RwLock<ChunkData>>, ChunkError> {
        if self.data.is_none() {
            return Err(ChunkError::Uninitialized(self.pos));
        }
        Ok(self.data.as_ref().unwrap().clone())
    }

    pub fn set_block(&mut self, pos: IVec3, state: BlockState) -> Result<BlockState, ChunkError> {
        if !self.is_initialized() {
            return Err(ChunkError::Uninitialized(self.pos));
        }
        let data = self.data.as_mut().unwrap();
        let mut write_lock = data.write().unwrap();
        write_lock.set_block(pos.x as usize, pos.y as usize, pos.z as usize, state)
    }

    pub fn get_block(&self, pos: IVec3) -> Result<BlockState, ChunkError> {
        if !self.is_initialized() {
            return Err(ChunkError::Uninitialized(self.pos));
        }
        let data = self.data.as_ref().unwrap();
        let read_lock = data.read().unwrap();
        read_lock.get_block(pos.x as usize, pos.y as usize, pos.z as usize)
    }

    pub fn get_pos(&self) -> IVec3 {
        self.pos
    }

    pub fn get_generation_status(&self) -> ChunkGenerationStatus {
        self.generation_status
    }

    pub fn is_initialized(&self) -> bool {
        self.data.is_some() && match self.generation_status {
            ChunkGenerationStatus::Generated => true,
            _ => false
        }
    }

    pub fn get_entity(&self) -> Entity {
        self.chunk_entity
    }

    pub fn init_data(&mut self, data: ChunkData) -> Result<(), ChunkError> {
        if self.data.is_some() {
            return Err(ChunkError::AlreadyInitialized(self.pos));
        }
        let _span = info_span!("chunk_init_data").entered();



        self.data = Some(Arc::new(RwLock::new(data)));

        //TODO: switch to AfterTerrain when implemented decorators
        self.generation_status = ChunkGenerationStatus::Generated;

        Ok(())
    }
}

#[derive(Default, Debug, Component)]
pub struct ChunkNeedsMeshing;


/// Marker component for chunk entities in the world. Contains the pos.
/// Chunks are separate entities while the World stores the chunk data, allowing easy lookups inside and outside of systems.
#[derive(Debug, Component)]
pub struct ChunkMarker {
    pos: IVec3
}
impl ChunkMarker {
    pub fn new(pos: IVec3) -> Self {
        Self { pos }
    }
    pub fn get_pos(&self) -> IVec3 {
        self.pos
    }
}


#[derive(Debug, Copy, Clone)]
pub enum ChunkGenerationStatus {
    NotGenerated,
    AfterTerrain,
    // todo: maybe remove
    AfterDecorations,
    Generated
}


// Representation of chunks in memory
// A chunk is a 32x32x32 region of the world which contains blocks and blockstates.
// In the future, they will also contain a "palette" for the different types of blocks in the world
// For now we'll just do a byte array with the data
#[derive(Debug, Clone)]
pub struct ChunkData {
    // number of bits per block.
    pub id_size: usize,
    // for now we'll do a vector of strings - later this will be a better id form
    palette: Vec<PaletteEntry>,
    // vec is heap allocated so this is fine
    pub data: BitVec,

    // if this is some: chunk is just one block. Can be air.
    is_single: bool,
}




impl ChunkData {


    // how many bits per ID. This can be 1 bit, 4 bits, one byte, etc. It depends on the size of the palette.
    pub const CHUNK_SIZE: usize = 32;
    pub const BLOCKS_PER_CHUNK: usize = Self::CHUNK_SIZE.pow(3);

    // generally do not create this yourself
    pub fn with_data(data: BitVec, palette: Vec<PaletteEntry>) -> Self {

        // calcualtes the closest power of two id size for the palette.
        let id_size = ((palette.len()) as f32).log2().ceil() as usize;
        if data.len() / id_size != Self::BLOCKS_PER_CHUNK {
            panic!("Bit data uses {} bits per block, but palette requires {} bits per block!", data.len() as f32 / Self::BLOCKS_PER_CHUNK as f32, id_size)
        }

        ChunkData {
            id_size,
            palette,
            data,
            is_single: false,
        }
    }

    pub fn single(state: BlockState) -> Self {
        let palette = vec![
            PaletteEntry::new(state),
        ];

        ChunkData {
            id_size: 1,
            data: BitVec::new(),
            palette,
            is_single: true,
        }
    }

    pub fn is_single(&self) -> bool {
        self.is_single
    }

    pub fn is_empty(&self) -> bool {
        self.is_single && self.palette[0].block.is_air()
    }

    // there are 32768 blocks in a chunk, so 32768 possible states. Could be stored in a u16 but eh.
    pub fn block_at(&self, x: usize, y: usize, z: usize) -> usize {
        let max = Self::CHUNK_SIZE;
        if x >= max || y >= max || z >= max {
            panic!("point {x}, {y}, {z} is out of bounds!");
        }

        // (i / depth) % width = x;
        // i / depth * width = y;
        // i % depth = z;
        // reverse: i = (depth * width * y) + (depth * x) + z
        // this is the raw index.
        let index = xyz_to_index(x, y, z);
        self.block_at_index(index)
    }

    pub fn block_at_index(&self, index: usize) -> usize {
        // now we need to multiply the index by the size of one ID.
        // This will point to the first bit of our id, then we read the next ID_SIZE bits.
        let scaled_index = index * self.id_size;

        // if single we just return either 1 (first palette id) or 0 if it's empty (air).
        if self.is_single {
            return self.palette.len();
        };

        block_at_raw(&self.data, self.id_size, scaled_index)
    }
    
    pub fn palette_iter(&self) -> Iter<'_, PaletteEntry> {
        self.palette.iter()
    }
    
    pub fn palette_len(&self) -> usize {
        self.palette.len()
    }

    pub fn lookup_palette(&self, index: usize) -> Result<&PaletteEntry, ChunkError> {
        Ok(&self.palette[index])
    }

    // adds palette to the entry and returns the id it adds at
    pub fn add_palette(&mut self, entry: PaletteEntry) -> usize {
        // do nothing if palette exists
        for i in 0..self.palette.len() {
            if self.palette[i] == entry {
                return i;
            }
        }

        if let Some(i) = self.first_free_palette() {
            self.palette[i] = entry;
            return i;
        }
        // no free palettes, add a new one.
        let max_palettes = 2_usize.pow(self.id_size as u32);
        // palettes are full. Resize the data.
        if (self.palette.len()) == max_palettes {
            self.grow_data();
        }
        // push palette at the end.
        self.palette.push(entry);
        // return last index
        self.palette.len() - 1
    }

    pub fn get_block(&self, x: usize, y: usize, z: usize) -> Result<BlockState, ChunkError> {
        if x >= ChunkData::CHUNK_SIZE || y >= ChunkData::CHUNK_SIZE || z >= ChunkData::CHUNK_SIZE {
            return Err(ChunkError::OutOfBounds(ivec3(x as i32, y as i32, z as i32)));
        }
        let id = self.block_at(x, y, z);
        Ok(self.palette[id].block.clone())
    }



    pub fn set_block(&mut self, x: usize, y: usize, z: usize, block: BlockState) -> Result<BlockState, ChunkError> {
        if x >= Self::CHUNK_SIZE || y >= Self::CHUNK_SIZE || z >= Self::CHUNK_SIZE {
            return Err(ChunkError::OutOfBounds(ivec3(x as i32, y as i32, z as i32)));
        }




        if self.is_single {
            // single block? not anymore!
            let has_to_expand = self.palette[0].block != block;

            if !has_to_expand {
                // no changes since we've set the block to the one block this chunk is entirely
                return Ok(block);
            }

            // need to make data now - since we're setting block lol.
            self.is_single = false;
            // fill with either 0 (air) or 1 (first palette entry)
            self.data = bitvec![self.palette.len(); Self::BLOCKS_PER_CHUNK];
            self.palette[0].ref_count = Self::BLOCKS_PER_CHUNK as u16;
        }


        let old_block = self.block_at(x, y, z);
        let index = xyz_to_index(x, y, z);

        // if old block is not air, we need to decrease refcount.
        let mut p = &mut self.palette[old_block];
        if p.is_free() {
            panic!("Invalid palette data: palette {:?} is free, but exists in data", p);
        }
        p.ref_count -= 1;
        let ret = p.block.clone();


        // check the palette to see if this block already is in it (including free ones!)
        for palette_idx in 0..self.palette.len() {
            let mut p = &mut self.palette[palette_idx];
            // block is already in the palette. TODO: update for blockstates.
            if p.block == block {
                p.ref_count += 1;


                // actually does the bit setting operation


                set_raw(&mut self.data, self.id_size, index * self.id_size, palette_idx);


                return Ok(ret);
            }
        }
        // block is not in the palette, add it to the palette.
        let block_id = self.add_palette(PaletteEntry::new(block));
        // increase palette's refcount
        self.palette[block_id].ref_count += 1;


        //update the raw data
        set_raw(&mut self.data, self.id_size, index * self.id_size, block_id);

        //return old block.
        Ok(ret)

    }


    // grows the internal data storage, realigning all the bit data
    fn grow_data(&mut self) {
        let old_size = self.id_size;
        // we always double the bit size.
        let new_size = old_size * 2;

        // make a new bitvec with our expected size.
        let mut new_vec = BitVec::with_capacity(new_size * Self::BLOCKS_PER_CHUNK);
        for i in 0..Self::BLOCKS_PER_CHUNK {

            // allocate new space required
            for _ in 0..(new_size - old_size) {
                new_vec.push(false);
            }

            // copy old data.
            new_vec.extend_from_bitslice(&self.data[i * old_size..i * old_size + old_size]);
        }
        self.data = new_vec;
        self.id_size = new_size;
    }


    // attempts to shrink data. Panics if shrinking would fail.
    fn shrink_data(&mut self) {

    }


    // returns the first free palette in the list, or none if all existing palette entries are active
    fn first_free_palette(&self) -> Option<usize> {
        if self.palette.len() == 0 {
            return None;
        }
        for i in 1..self.palette.len() {
            if self.palette[i].is_free() {
                return Some(i);
            }
        }
        None
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct PaletteEntry {
    // 32768 possible blocks
    ref_count: u16,
    pub block: BlockState,
}

impl PaletteEntry {
    pub fn new(state: BlockState) -> Self {
        PaletteEntry {
            block: state,
            ref_count: 0,
        }
    }

    pub fn is_free(&self) -> bool {
        self.ref_count == 0
    }

    pub fn increment_ref_count(&mut self) {
        self.ref_count += 1;
    }
    pub fn decrement_ref_count(&mut self) {
        if self.ref_count == 0 {
            panic!("Palette is already free, cannot decrement refcount!");
        }
        self.ref_count -= 1;
    }
}

fn block_at_raw(data: &BitVec, id_size: usize, scaled_index: usize) -> usize {
    let value = &data[scaled_index..scaled_index + id_size];
    // if for some god forsaken reason the length of this data is somehow longer than 32, crash and burn
    assert!(value.len() <= 32);

    // folds the bit array into an integer and returns
    math::bslice_to_usize(value)
}

fn set_raw(data: &mut BitVec, id_size: usize, scaled_index: usize, value: usize) {

    let arr = value.into_bitarray::<Msb0>();
    let slice = &arr[size_of::<usize>() * 8 - id_size..size_of::<usize>() * 8];
    for i in 0..id_size {
        data.set(scaled_index + i, slice[i]);
    }
}

fn xyz_to_index(x: usize, y: usize, z: usize) -> usize {
    // reverse: i = (depth * width * y) + (depth * x) + z
    let max = ChunkData::CHUNK_SIZE;
    (max * max * y) + (max * x) + z
}


pub fn chunk_pos_to_transform(pos: IVec3) -> Transform {
    Transform::from_xyz((pos.x * ChunkData::CHUNK_SIZE as i32) as f32, (pos.y * ChunkData::CHUNK_SIZE as i32) as f32, (pos.z * ChunkData::CHUNK_SIZE as i32) as f32)
}
pub fn chunk_pos_to_world_pos(pos: IVec3) -> IVec3 {
    pos * ChunkData::CHUNK_SIZE as i32
}
pub fn transform_to_chunk_pos(transform: &Transform) -> IVec3 {
    let vec = transform.translation.as_block_pos();
    pos_to_chunk_pos(vec)
}
pub fn pos_to_chunk_pos(pos: IVec3) -> IVec3 {
    let vec = pos.as_vec3();
    ivec3((vec.x / ChunkData::CHUNK_SIZE as f32).floor() as i32, (vec.y / ChunkData::CHUNK_SIZE as f32).floor() as i32, (vec.z / ChunkData::CHUNK_SIZE as f32).floor() as i32)
}

pub fn pos_to_chunk_local(pos: IVec3) -> IVec3 {
    pos - (ChunkData::CHUNK_SIZE as i32 * pos_to_chunk_pos(pos))
}

#[derive(Component)]
pub struct ChunkMeshMarker;