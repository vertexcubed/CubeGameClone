use crate::render::block::{BlockModelMinimal, MeshDataCache};
use crate::render::material::BlockMaterial;
use crate::world::block::{BlockState, Direction};
use crate::world::chunk;
use crate::world::chunk::ChunkData;
use bevy::asset::RenderAssetUsages;
use bevy::log::info_span;
use bevy::math::vec3;
use bevy::prelude::Mesh;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use std::collections::HashMap;

#[derive(Debug, Copy, Clone, PartialEq)]
enum Facing {
    North, // +z
    South, // -z
    East, // +x
    West, // -x
    Up, // +y
    Down, // -y
}

impl From<Direction> for Facing {
    fn from(value: Direction) -> Self {
        match value {
            Direction::Up => Facing::Up,
            Direction::Down => Facing::Down,
            Direction::North => Facing::North,
            Direction::South => Facing::South,
            Direction::East => Facing::East,
            Direction::West => Facing::West,
        }
    }
}

pub type NeighborData<'a> = (&'a ChunkData, &'a ChunkData, &'a ChunkData, &'a ChunkData, &'a ChunkData, &'a ChunkData);

pub fn create_chunk_mesh(
    chunk: &ChunkData,
    cache: &MeshDataCache,
    neighbors: Option<NeighborData>
) -> Mesh {

    let span = info_span!("create_chunk_mesh").entered();

    let model_map = cache.inner.load();
    
    let mut positions = Vec::<[f32; 3]>::new();
    let mut uv0s = Vec::<[f32; 2]>::new();
    let mut normals = Vec::<[f32; 3]>::new();
    let mut indices = Vec::<u32>::new();
    let mut texture_ids = Vec::<u32>::new();
    

    //TODO: optimize in the case of single chunks (chunks made up of just one block)
    
    let mut indices_offset = 0;
    
    for i in 0..ChunkData::BLOCKS_PER_CHUNK {

        let id = chunk.block_at_index(i);
        
        let block_id = chunk.lookup_palette(id).unwrap();
        if block_id.block.is_air() {
            continue;
        }
        let block_model = model_map.get(&block_id.block).unwrap();

        let (x, y, z) = index_to_xyz(i);

        // iter over each face
        for face in block_model.face_iter() {
            if let Some(dir) = face.get_cull_mode() {
                // cull face - block adjacent is solid
                if !should_make_face(dir.into(), &chunk, x, y, z, neighbors, &model_map) {
                    continue;
                }
            }
            // else make a new face
            let (
                mut face_pos, 
                mut face_uv0, 
                mut face_normal, 
                mut face_index, 
                mut face_texture_ids
            ) = face.get_face_data(vec3(x as f32, y as f32, z as f32), indices_offset);

            indices_offset += face_pos.len() as u32;

            positions.append(&mut face_pos);
            uv0s.append(&mut face_uv0);
            normals.append(&mut face_normal);
            indices.append(&mut face_index);
            texture_ids.append(&mut face_texture_ids);

        }
    }
    
    // creates the chunk mesh
    let ret = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(BlockMaterial::ATTRIBUTE_ARRAY_ID, texture_ids)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv0s)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_indices(Indices::U32(indices));
    
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

fn should_make_face(
    facing: Facing,
    chunk: &ChunkData,
    x: usize, y: usize, z: usize,
    neighbors: Option<NeighborData>,
    model_map: &HashMap<BlockState, BlockModelMinimal>
) -> bool {

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
    let (block, queried_chunk) = if neighbors.is_some() {
        let (north, south, east, west, up, down) = neighbors.unwrap();

        if new_z < 0 {
            (south.block_at(new_x as usize, new_y as usize, last), south)
        }
        else if new_z > last as isize {
            (north.block_at(new_x as usize, new_y as usize, 0), north)
        }
        else if new_x < 0 {
            (west.block_at(last, new_y as usize, new_z as usize), west)
        }
        else if new_x > last as isize {
            (east.block_at(0, new_y as usize, new_z as usize), east)
        }
        else if new_y < 0 {
            (down.block_at(new_x as usize, last, new_z as usize), down)
        }
        else if new_y > last as isize {
            (up.block_at(new_x as usize, 0, new_z as usize), up)
        }
        else {
            (chunk.block_at(new_x as usize, new_y as usize, new_z as usize), chunk)
        }
    }
    else {
        (chunk.block_at(new_x as usize, new_y as usize, new_z as usize), chunk)
    };
    
    let queried_side = match facing {
        Facing::North => Direction::South,
        Facing::South => Direction::North,
        Facing::East => Direction::West,
        Facing::West => Direction::East,
        Facing::Up => Direction::Down,
        Facing::Down => Direction::Up,
    };
    
    let state = &queried_chunk.lookup_palette(block).unwrap().block;
    let model = model_map.get(&state);
    // no model? treat like air
    if model.is_none() {
        return true;
    }
    
    !model.unwrap().is_full(queried_side)
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

fn index_to_xyz(i: usize) -> (usize, usize, usize) {
    (
        (i / ChunkData::CHUNK_SIZE) % ChunkData::CHUNK_SIZE,
        i / (ChunkData::CHUNK_SIZE * ChunkData::CHUNK_SIZE),
        i % ChunkData::CHUNK_SIZE
    )
}