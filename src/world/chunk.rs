use std::slice::Iter;
use std::sync::{Arc, RwLock};
use bevy::ecs::error::panic;
use bevy::log::info_span;
use crate::core::errors::ChunkError;
use crate::math::block::Vec3Ext;
use crate::world::block::BlockState;
use bevy::math::ivec3;
use bevy::prelude::{Component, Entity, IVec3, Transform};
use serde::{Deserialize, Serialize};

/// A data structure that represents a chunk in the world. Stores some information about it tied to
/// its physical state, like the blocks in the chunk and its state.
#[derive(Debug, Clone)]
pub struct Chunk {
    /// The position of this chunk in the world. Should always be the same.
    pos: IVec3,
    /// Data may be read by multiple threads, but only modified by one thread.
    /// Structure somewhat mirrors how chunks are stored to disk
    /// Note: this may not be available! Especially if the chunk is not generated yet.
    data: Option<Arc<RwLock<ChunkData>>>,
    /// The entity ID of the corresponding entity.
    /// The entity stores all mesh information and rendering data and other in world data
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


/// Representation of chunks in memory
/// A chunk is a 32x32x32 region of the world which contains blocks and blockstates.
#[derive(Debug, Clone)]
pub struct ChunkData {
    palette: Vec<PaletteEntry>,
    /// Important: when 2 bytes are used per block, data is stored Little Endian! This means the first byte is the LSB and the second byte is the MSB
    pub data: Vec<u8>,

    /// if this is true: chunk is just one block. Can be air.
    is_single: bool,
    /// if true, blocks use two bytes per id rather than one. only in the case where > 256 different blocks in a chunk
    double_bytes: bool
}




impl ChunkData {

    // how many bits per ID. This can be 1 bit, 4 bits, one byte, etc. It depends on the size of the palette.
    pub const CHUNK_SIZE: usize = 32;
    pub const BLOCKS_PER_CHUNK: usize = Self::CHUNK_SIZE.pow(3);

    pub const DOUBLE_BLOCKS_PER_CHUNK: usize = Self::BLOCKS_PER_CHUNK * 2;

    // generally do not create this yourself
    pub fn with_data(data: Vec<u8>, palette: Vec<PaletteEntry>) -> Self {

        // calcualtes the closest power of two id size for the palette.
        let double_bytes = palette.len() > 256;
        match (data.len(), double_bytes) {
            (Self::BLOCKS_PER_CHUNK, false) | (Self::DOUBLE_BLOCKS_PER_CHUNK, true) => {},
            _ => {
                let len = if double_bytes { Self::DOUBLE_BLOCKS_PER_CHUNK } else { Self::BLOCKS_PER_CHUNK };
                panic!("Invalid size of data. Must be {len} bytes long!")
            }
        }

        ChunkData {
            palette,
            data,
            double_bytes,
            is_single: false,
        }
    }

    pub fn single(state: BlockState) -> Self {
        let palette = vec![
            PaletteEntry::new(state),
        ];

        ChunkData {
            data: Vec::new(),
            double_bytes: false,
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
        // if single we just return 0
        if self.is_single {
            return 0
        }

        if self.double_bytes {
            let scaled_index = index * 2;
            ((self.data[scaled_index + 1] as usize) << 8) | (self.data[scaled_index] as usize)
        }
        else {
            self.data[index] as usize
        }
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
        // palettes are full. Resize the data.
        if (self.palette.len()) == 256 {
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


        let old_block = self.block_at(x, y, z);
        let index = xyz_to_index(x, y, z);

        // if we're setting the block to the same block, return old and do nothing
        if self.palette[old_block].block == block {
            return Ok(block);
        }

        // if single block chunk, now we need to init data and expand
        if self.is_single {

            // need to make data now - since we're setting block lol.
            self.is_single = false;
            // init data to a vec of 0s
            self.data = vec![0; Self::CHUNK_SIZE];
            // set refcount to 32768
            self.palette[0].ref_count = Self::BLOCKS_PER_CHUNK as u16;
        }


        //Grab the old block and decrease the refcount.
        let mut p = &mut self.palette[old_block];
        if p.is_free() {
            panic!("Invalid palette data: palette {:?} is free, but exists in data", p);
        }
        p.ref_count -= 1;
        let ret = p.block.clone();


        // check the palette to see if this block already is in it (including free ones!)
        for palette_idx in 0..self.palette.len() {
            let mut p = &mut self.palette[palette_idx];
            // block is already in the palette, so just increase the refcount and set the data.
            if p.block == block {
                p.ref_count += 1;
                self.set_raw(index, palette_idx);
                return Ok(ret);
            }
        }
        // block is not in the palette, add it to the palette.
        let block_id = self.add_palette(PaletteEntry::new(block));
        // increase palette's refcount
        self.palette[block_id].ref_count += 1;

        //update the raw data
        self.set_raw(index, block_id);

        //return old block.
        Ok(ret)

    }

    pub fn set_raw(&mut self, index: usize, block_id: usize) {
        if self.is_single {
            panic!("Cannot set raw on single chunks!")
        }
        if self.double_bytes {
            let lsb = block_id as u8;
            let msb = (block_id >> 8) as u8;
            let scaled_index = index * 2;
            self.data[scaled_index] = lsb;
            self.data[scaled_index + 1] = msb;
        }
        else {
            self.data[index] = block_id as u8
        }
    }


    // grows the data from 1 byte per block to 2 bytes per block. Panics if data is already 2 bytes per block
    fn grow_data(&mut self) {
        if self.double_bytes {
            panic!("Cannot grow chunk data that is already double byte!")
        }
        let mut new_vec = Vec::with_capacity(Self::DOUBLE_BLOCKS_PER_CHUNK);
        for i in 0..Self::BLOCKS_PER_CHUNK {
            // LSB
            new_vec.push(self.data[i]);
            // MSB
            new_vec.push(0);
        }
        self.data = new_vec;
        self.double_bytes = true;
    }


    // attempts to shrink data. Panics if shrinking would fail.
    fn shrink_data(&mut self) {
        if !self.double_bytes {
            panic!("Cannot shrink chunk data that is only single byte!")
        }
        todo!("Shrinking not yet Implemented")
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

// fn block_at_raw(data: &BitVec, id_size: usize, scaled_index: usize) -> usize {
//     let value = &data[scaled_index..scaled_index + id_size];
//     // if for some god forsaken reason the length of this data is somehow longer than 32, crash and burn
//     assert!(value.len() <= 32);
//
//     // folds the bit array into an integer and returns
//     math::bslice_to_usize(value)
// }

// fn set_raw(data: &mut BitVec, id_size: usize, scaled_index: usize, value: usize) {
//
//     let arr = value.into_bitarray::<Msb0>();
//     let slice = &arr[size_of::<usize>() * 8 - id_size..size_of::<usize>() * 8];
//     for i in 0..id_size {
//         data.set(scaled_index + i, slice[i]);
//     }
// }

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


/// A packed representation of ChunkData. Fits the data itself into as little u64s as it can.
/// Other than that, functionally the same.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PackedChunkData {
    palette: Vec<PackedPaletteEntry>,
    /// Important: values are stored from LSB -> MSB.
    block_data: Vec<u64>,
    is_single: bool
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PackedPaletteEntry {
    ref_count: u16,
    block: BlockState,
}
impl From<PaletteEntry> for PackedPaletteEntry {
    fn from(value: PaletteEntry) -> Self {
        PackedPaletteEntry {
            ref_count: value.ref_count,
            block: value.block
        }
    }
}
impl Into<PaletteEntry> for PackedPaletteEntry {
    fn into(self) -> PaletteEntry {
        PaletteEntry {
            ref_count: self.ref_count,
            block: self.block,
        }
    }
}
impl From<ChunkData> for PackedChunkData {
    fn from(value: ChunkData) -> Self {
        Self::from(&value)
    }
}


impl From<&ChunkData> for PackedChunkData {
    fn from(value: &ChunkData) -> Self {

        // single chunks are easy.
        if value.is_single {
            //TODO: move to ChunkData validate function
            if value.palette.len() != 1 {
                panic!("Malformed ChunkData: data marked as single, but palette length is not 1!");
            }
            if value.palette[0].ref_count as usize != ChunkData::BLOCKS_PER_CHUNK {
                panic!("Malformed ChunkData: data marked as single must have refcount of {}", ChunkData::BLOCKS_PER_CHUNK)
            }

            return Self {
                block_data: Vec::new(),
                palette: vec![value.palette[0].clone().into()],
                is_single: true
            }
        }

        let palette: Vec<PackedPaletteEntry> = value.palette.iter().filter_map(|entry| {
            // trim empty palette entries
            if entry.ref_count == 0 { None } else { Some(entry.clone().into()) } //TODO: remove clone
        }).collect::<Vec<_>>();


        // number of bits per id to use, rounded to power of 2
        // ugly ass formula but idk a better way of simplifying this
        let id_size = 2_usize.pow(
            f32::ceil(
                f32::log2(
                    f32::log2(
                        (palette.len() as f32)
                    )
                )
            ) as u32
        ).max(1); // sets to 1 in the case id_size = 1

        println!("Id size: {}. Palette length: {}.", id_size, palette.len());


        let mut packed_data = Vec::<u64>::with_capacity(id_size * 32768 / 64);

        // values will be stored in
        let mut quad_word = 0_u64;
        let mut bit_pointer = 0;

        for i in 0..ChunkData::BLOCKS_PER_CHUNK {
            // grabs the block id regardless of double_bytes or not
            let id = value.block_at_index(i);

            // creates a bit mask - for example, if we need 4 bits per block, we get 2^4 - 1 = 15 = 0b1111
            let mask = 2_usize.pow(id_size as u32) - 1;
            // shift and create the data.
            let to_add: u64 = ((id & mask) as u64) << bit_pointer;
            //now we add it to our qword
            quad_word |= to_add;
            //now increase bit_pointer
            bit_pointer += id_size;
            // if bit_pointer = 64, we've filled this qword. Push to the vec and then set back to 0
            if bit_pointer >= 64 {
                packed_data.push(quad_word);
                quad_word = 0;
                bit_pointer = 0;
            }
        }

        Self {
            block_data: packed_data,
            palette,
            is_single: false
        }
    }
}
impl Into<ChunkData> for PackedChunkData {
    fn into(self) -> ChunkData {
        // move everything out
        let (palette, block_data, is_single) = (self.palette, self.block_data, self.is_single);

        if is_single {
            //TODO: move to ChunkData validate function
            if palette.len() != 1 {
                panic!("Malformed saved chunk data: data marked as single, but palette length is not 1!");
            }
            if palette[0].ref_count as usize != ChunkData::BLOCKS_PER_CHUNK {
                panic!("Malformed saved chunk data: data marked as single must have refcount of {}", ChunkData::BLOCKS_PER_CHUNK)
            }
            return ChunkData {
                palette: vec![palette[0].clone().into()],
                data: Vec::new(),
                is_single: true,
                double_bytes: false
            }
        }
        // we don't discard 0 size palettes
        let palette: Vec<PaletteEntry> = palette.into_iter().map(|entry| entry.into()).collect::<Vec<_>>();

        // number of bits per id to use, rounded to power of 2
        // ugly ass formula but idk a better way of simplifying this
        let id_size = 2_usize.pow(
            f32::ceil(
                f32::log2(
                    f32::log2(
                        (palette.len() as f32)
                    )
                )
            ) as u32
        ).max(1); // sets to 1 in the case id_size = 1

        let double_bytes = palette.len() > 256;
        let vec_size = if double_bytes { ChunkData::DOUBLE_BLOCKS_PER_CHUNK } else { ChunkData::BLOCKS_PER_CHUNK };
        let mut unpacked_data: Vec<u8> = Vec::with_capacity(vec_size);

        let mut qword_index = 0;
        let mut bit_pointer = 0;
        while qword_index < block_data.len() {
            let quad_word = block_data[qword_index];

            // creates a bit mask - for example, if we need 4 bits per block, we get 2^4 - 1 = 15 = 0b1111
            let mask = 2_u64.pow(id_size as u32) - 1;
            // shift the mask, grab values, then shift back so its aligned at 0.
            let block_id: usize = (((mask << bit_pointer) & quad_word) >> bit_pointer) as usize;

            if double_bytes {
                let lsb = block_id as u8;
                let msb = (block_id >> 8) as u8;
                unpacked_data.push(lsb);
                unpacked_data.push(msb);
            }
            else {
                unpacked_data.push(block_id as u8);
            }
            // increment bit_pointer
            bit_pointer += id_size;
            // if bit_pointer = 64, we've read everything in this qword. Move on to the next qword
            if bit_pointer >= 64 {
                qword_index += 1;
                bit_pointer = 0;
            }
        }
        assert_eq!(unpacked_data.len(), vec_size);

        ChunkData {
            data: unpacked_data,
            palette,
            is_single,
            double_bytes,
        }
    }
}