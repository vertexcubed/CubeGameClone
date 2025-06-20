use crate::core::errors::ChunkError;
use crate::render::material::BlockMaterial;
use crate::render::MeshDataCache;
use crate::world::block::BlockState;
use bevy::asset::RenderAssetUsages;
use bevy::math::{ivec3, vec3, Vec3};
use bevy::prelude::{Component, IVec3, Mesh, Transform};
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bitvec::bitvec;
use bitvec::field::BitField;
use bitvec::order::Msb0;
use bitvec::prelude::BitVec;
use bitvec::view::BitViewSized;
use std::sync::{Arc, RwLock};
use std::time::Instant;

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

#[derive(Debug, Clone, Component)]
pub struct ChunkComponent {
    // data may be read by multiple threads, but only modified by one thread
    data: Arc<RwLock<ChunkData>>,
    pub pos: IVec3,
}
impl ChunkComponent {
    pub fn new(pos: IVec3, data: ChunkData) -> Self {
        Self {
            data: Arc::new(RwLock::new(data)),
            pos,
        }
    }
    pub fn borrow_data(&self) -> Arc<RwLock<ChunkData>> {
        self.data.clone()
    }

    pub fn set_block(&mut self, pos: IVec3, state: BlockState) -> Result<BlockState, ChunkError> {
        let mut write = self.data.write().unwrap();
        write.set_block(pos.x as usize, pos.y as usize, pos.z as usize, state)
    }

    pub fn get_block(&self, pos: IVec3) -> BlockState {
        let read = self.data.read().unwrap();
        read.get_block(pos.x as usize, pos.y as usize, pos.z as usize)
    }
}
#[derive(Default, Debug, Component)]
pub struct ChunkNeedsMeshing;



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
    pub fn new(data: BitVec, palette: Vec<PaletteEntry>) -> Self {

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
            is_single: true
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

    pub fn get_block(&self, x: usize, y: usize, z: usize) -> BlockState {
        let id = self.block_at(x, y, z);
        self.palette[id].block.clone()
    }



    pub fn set_block(&mut self, x: usize, y: usize, z: usize, block: BlockState) -> Result<BlockState, ChunkError> {
        if x >= Self::CHUNK_SIZE || y >= Self::CHUNK_SIZE || z >= Self::CHUNK_SIZE {
            let message = format!("Index {x}, {y}, {z} is out of bounds.");
            return Err(ChunkError::new(message.as_str()));
        }
        


        
        if self.is_single {
            // single block? not anymore!
            let has_to_expand = self.palette[0].block != block;
            
            if !has_to_expand {
                // no changes since we've set the block to the one block this chunk is entirely
                println!("No changes, chunk is single");
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


        println!("Block to place: {:?}", block);
        println!("Palette: {:?}", self.palette);
        // check the palette to see if this block already is in it (including free ones!)
        for palette_idx in 0..self.palette.len() {
            let mut p = &mut self.palette[palette_idx];
            // block is already in the palette. TODO: update for blockstates.
            if p.block == block {
                p.ref_count += 1;

                println!("Found stone: index: {}", palette_idx);
                println!("Old id: {}", old_block);

                // actually does the bit setting operation

                println!("Old data: {}", &self.data[index * self.id_size..index * self.id_size + self.id_size]);

                set_raw(&mut self.data, self.id_size, index * self.id_size, palette_idx + 1);

                println!("New data: {}", &self.data[index * self.id_size..index * self.id_size + self.id_size]);

                return Ok(ret);
            }
        }
        // block is not in the palette, add it to the palette.
        let block_id = self.add_palette(PaletteEntry::new(block));
        // increase palette's refcount
        self.palette[block_id].ref_count += 1;

        //update the raw data
        set_raw(&mut self.data, self.id_size, index * self.id_size, block_id + 1);

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
            new_vec.extend_from_bitslice(&self.data[i..i+old_size]);
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

fn block_at_raw(data: &BitVec, id_size: usize, scaled_index: usize) -> usize {
    let value = &data[scaled_index..scaled_index + id_size];
    // if for some god forsaken reason the length of this data is somehow longer than 32, crash and burn
    assert!(value.len() <= 32);

    // folds the bit array into an integer and returns
    let out = value.into_iter().fold(0, |acc, b| {
        let bit: bool = b.as_ref().clone();
        (acc << 1) + (bit as usize)
    });

    out
}

fn set_raw(data: &mut BitVec, id_size: usize, scaled_index: usize, value: usize) {

    let arr = value.into_bitarray::<Msb0>();
    let slice = &arr[size_of::<usize>() * 8 - id_size..size_of::<usize>() * 8];
    for i in 0..id_size {
        data.set(scaled_index + i, slice[i]);
    }
}

fn index_to_xyz(i: usize) -> (usize, usize, usize) {
    (
        (i / ChunkData::CHUNK_SIZE) % ChunkData::CHUNK_SIZE,
        i / (ChunkData::CHUNK_SIZE * ChunkData::CHUNK_SIZE),
        i % ChunkData::CHUNK_SIZE
    )
}

fn xyz_to_index(x: usize, y: usize, z: usize) -> usize {
    // reverse: i = (depth * width * y) + (depth * x) + z
    let max = ChunkData::CHUNK_SIZE;
    (max * max * y) + (max * x) + z
}


pub fn chunk_pos_to_transform(pos: IVec3) -> Transform {
    Transform::from_xyz((pos.x * ChunkData::CHUNK_SIZE as i32) as f32, (pos.y * ChunkData::CHUNK_SIZE as i32) as f32, (pos.z * ChunkData::CHUNK_SIZE as i32) as f32)
}
pub fn transform_to_chunk_pos(transform: &Transform) -> IVec3 {
    let vec = transform.translation;
    pos_to_chunk_pos(vec.as_ivec3())
}
pub fn pos_to_chunk_pos(pos: IVec3) -> IVec3 {
    let vec = pos.as_vec3();
    ivec3((vec.x / ChunkData::CHUNK_SIZE as f32).floor() as i32, (vec.y / ChunkData::CHUNK_SIZE as f32).floor() as i32, (vec.z / ChunkData::CHUNK_SIZE as f32).floor() as i32)
}

pub fn pos_to_chunk_local(pos: IVec3) -> IVec3 {
    pos - (32 * pos_to_chunk_pos(pos))
}

//===============
// - mesh stuff -
//===============



#[derive(Debug, Copy, Clone, PartialEq)]
enum Facing {
    North, // +z
    South, // -z
    East, // +x
    West, // -x
    Up, // +y
    Down, // -y
}


pub type NeighborData<'a> = (&'a ChunkData, &'a ChunkData, &'a ChunkData, &'a ChunkData,&'a ChunkData, &'a ChunkData);

pub fn create_chunk_mesh(
    chunk: &ChunkData,
    cache: &MeshDataCache,
    neighbors: Option<NeighborData>
) -> Mesh {

    let now = Instant::now();

    // info!("Creating chunk mesh.");

    let model_map = cache.inner.load();
    
    // faces to make a mesh for
    let mut faces = Vec::<(Facing, Vec3, u32)>::new();

    let mut positions = Vec::<[f32; 3]>::new();
    let mut uv0s = Vec::<[f32; 2]>::new();
    let mut normals = Vec::<[f32; 3]>::new();
    let mut indices = Vec::<u32>::new();
    let mut face_ids = Vec::<u32>::new();

    
    // make sure that index points to the first bit of the id.
    // let mut index = (working_data.leading_zeros() / chunk.id_size) * chunk.id_size;
    // debug!("Length of working_data: {}", working_data.len());

    //TODO: optimize in the case of single chunks (chunks made up of just one block)
    
    for i in 0..ChunkData::BLOCKS_PER_CHUNK {

        let id = chunk.block_at_index(i);
        
        let block_id = chunk.lookup_palette(id).unwrap();
        if block_id.block.is_air() {
            continue;
        }
        let block_model = model_map.get(&block_id.block).unwrap();
        let array_id = block_model.index;

        // println!("Block is {:?}", block);


        // println!("Block is {:?}", block);
        let (x, y, z) = index_to_xyz(i);
        // println!("{x}, {y}, {z}, {index}");


        if should_make_face(Facing::North, &chunk, x, y, z, neighbors) {
            faces.push((Facing::North, vec3(x as f32, y as f32, z as f32), array_id));
        }
        if should_make_face(Facing::South, &chunk, x, y, z, neighbors) {
            faces.push((Facing::South, vec3(x as f32, y as f32, z as f32), array_id));
        }
        if should_make_face(Facing::East, &chunk, x, y, z, neighbors) {
            faces.push((Facing::East, vec3(x as f32, y as f32, z as f32), array_id));
        }
        if should_make_face(Facing::West, &chunk, x, y, z, neighbors) {
            faces.push((Facing::West, vec3(x as f32, y as f32, z as f32), array_id));
        }
        if should_make_face(Facing::Up, &chunk, x, y, z, neighbors) {
            faces.push((Facing::Up, vec3(x as f32, y as f32, z as f32), array_id));
        }
        if should_make_face(Facing::Down, &chunk, x, y, z, neighbors) {
            faces.push((Facing::Down, vec3(x as f32, y as f32, z as f32), array_id));
        }
    }
    
    
    // while (index + chunk.id_size) < working_data.len() {
    // 
    //     
    // 
    //     // zero out nth block and move on.
    //     for j in 0..chunk.id_size {
    //         working_data.set(index + j, false);
    //     }
    //     index = (working_data.leading_zeros() / chunk.id_size) * chunk.id_size;
    // }

    let after_faces = now.elapsed().as_secs_f64();


    let mut index_offset = 0;
    for (dir, pos_offset, array_id) in faces {

        // face data
        let (face_pos, face_uv0, face_normal, face_index) = face_data(dir);
        
        // offsets and adds vertices
        for j in 0..4 {
            let (pos, uv0, normal) = (face_pos[j], face_uv0[j], face_normal[j]);
            // add offset for pos
            let new_pos = [pos[0] + pos_offset.x, pos[1] + pos_offset.y, pos[2] + pos_offset.z];
            positions.push(new_pos);
            uv0s.push(uv0);
            normals.push(normal);
            // array texture id
            face_ids.push(array_id);
        }
        // add index offset for indices
        for j in 0..6 {
            indices.push(face_index[j] + index_offset);
        }

        index_offset += 4;
    }
    
    // info!("Finished creating chunk mesh");

    let after_vertices = now.elapsed().as_secs_f64();

    // creates the chunk mesh
    let ret = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(BlockMaterial::ATTRIBUTE_ARRAY_ID, face_ids)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv0s)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_indices(Indices::U32(indices));

    let last = now.elapsed().as_secs_f64();

    // let clone = after_clone;
    // let face = after_faces - after_clone;
    // let vert = after_vertices - after_faces;
    // let mesh = last - after_vertices;
    // info!("Clone took {}. Face took {}. Vert took {}. Mesh took {}.", clone, face, vert, mesh);

    ret
}


type FaceData = ([[f32; 3]; 4], [[f32; 2]; 4], [[f32; 3]; 4], [u32; 6]);

// outputs vertex specific data for this block and face
fn face_data(facing: Facing) -> FaceData {
    match facing {
        Facing::North => (
            [ [0., 0., 1.], [0., 1., 1.], [1., 1., 1.], [1., 0., 1.], ],
            [ [0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0], ],
            [ [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], ],
            [ 0,3,1, 1,3,2, ],
        ),
        Facing::South => (
            [ [0., 0., 0.], [0., 1., 0.], [1., 1., 0.], [1., 0., 0.], ],
            [ [0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0], ],
            [ [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], ],
            [ 0,1,3, 1,2,3, ],
        ),
        Facing::East => (
            [ [1., 0., 0.], [1., 0., 1.], [1., 1., 1.], [1., 1., 0.], ],
            [ [1.0, 1.0], [0.0, 1.0], [0.0, 0.0], [1.0, 0.0], ],
            [ [1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 0.0], ],
            [ 0,3,1, 1,3,2, ],
        ),
        Facing::West => (
            [ [0., 0., 0.], [0., 0., 1.], [0., 1., 1.], [0., 1., 0.], ],
            [ [1.0, 1.0], [0.0, 1.0], [0.0, 0.0], [1.0, 0.0], ],
            [ [-1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], ],
            [ 0,1,3, 1,2,3 ],
        ),
        Facing::Up => (
            [ [0., 1., 0.], [1., 1., 0.], [1., 1., 1.], [0., 1., 1.], ],
            [ [0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0] ],
            [ [0.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 1.0, 0.0], ],
            [ 0,3,1, 1,3,2 ],
        ),
        Facing::Down => (
            [ [0., 0., 0.], [1., 0., 0.], [1., 0., 1.], [0., 0., 1.], ],
            [ [0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0], ],
            [ [0.0, -1.0, 0.0], [0.0, -1.0, 0.0], [0.0, -1.0, 0.0], [0.0, -1.0, 0.0], ],
            [ 0,1,3, 1,2,3 ]
        ),
    }
}

fn should_make_face(facing: Facing, chunk: &ChunkData, x: usize, y: usize, z: usize, neighbors: Option<NeighborData>) -> bool {

    let last = ChunkData::CHUNK_SIZE - 1;
    
    if neighbors.is_none() {
        match facing {
            Facing::North => {
                if(z == last) { return true; };
            }
            Facing::South => {
                if(z == 0) { return true; };
            }
            Facing::East => {
                if(x == last) { return true; };
            }
            Facing::West => {
                if(x == 0) { return true; };
            }
            Facing::Up => {
                if(y == last) { return true; };
            }
            Facing::Down => {
                if(y == 0) { return true; };
            }
        };
    };

    // get the value at new_pos
    let (new_x, new_y, new_z) = new_block(facing, x as isize, y as isize, z as isize);
    // Check west data.
    if neighbors.is_some() {
        let (north, south, east, west, up, down) = neighbors.unwrap();

        let block = if new_z < 0 {
            south.block_at(new_x as usize, new_y as usize, last)
        }
        else if new_z > last as isize {
            north.block_at(new_x as usize, new_y as usize, 0)
        }
        else if new_x < 0 {
            west.block_at(last, new_y as usize, new_z as usize)
        }
        else if new_x > last as isize {
            east.block_at(0, new_y as usize, new_z as usize)
        }
        else if new_y < 0 {
            down.block_at(new_x as usize, last, new_z as usize)
        }
        else if new_y > last as isize {
            up.block_at(new_x as usize, 0, new_z as usize)
        }
        else {
            chunk.block_at(new_x as usize, new_y as usize, new_z as usize)
        };
        block == 0
    }
    else {
        let block = chunk.block_at(new_x as usize, new_y as usize, new_z as usize);
        block == 0
    }
    
}

// no guarantee these are in bounds
fn new_block(facing: Facing, x: isize, y: isize, z: isize) -> (isize, isize, isize) {
    match facing {
        Facing::North => (x, y, z + 1),
        Facing::South => (x, y, z - 1),
        Facing::East =>  (x + 1, y, z),
        Facing::West =>  (x - 1, y, z),
        Facing::Up =>    (x, y + 1, z),
        Facing::Down =>  (x, y - 1, z),
    }
}
