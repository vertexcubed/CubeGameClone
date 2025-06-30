use crate::render::block::{BlockModelMinimal, FaceMinimal, MeshDataCache};
use crate::render::material::BlockMaterial;
use crate::world::chunk;
use crate::world::chunk::ChunkData;
use bevy::asset::RenderAssetUsages;
use bevy::log::info_span;
use bevy::math::{vec3, IVec3};
use bevy::prelude::{info, ivec3, Mesh};
use bevy::render::mesh::{Indices, PrimitiveTopology};
use std::collections::HashMap;
use std::time::Instant;
use crate::world::block::{BlockState, Direction};

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
    neighbors: NeighborData
) -> Mesh {

    // let span = info_span!("create_chunk_mesh").entered();

    let model_map = cache.inner.as_ref();
    
    let mut positions = Vec::<[f32; 3]>::with_capacity(ChunkData::BLOCKS_PER_CHUNK);
    let mut uv0s = Vec::<[f32; 2]>::with_capacity(ChunkData::BLOCKS_PER_CHUNK);
    let mut normals = Vec::<[f32; 3]>::with_capacity(ChunkData::BLOCKS_PER_CHUNK);
    let mut indices = Vec::<u32>::with_capacity(ChunkData::BLOCKS_PER_CHUNK);
    let mut texture_ids = Vec::<u32>::with_capacity(ChunkData::BLOCKS_PER_CHUNK);


    //TODO: optimize in the case of single chunks (chunks made up of just one block)

    let now = Instant::now();


    let mut indices_offset = 0;

    let mut cull_info = Vec::new();

    let mut faces: Vec<(IVec3, &FaceMinimal)> = Vec::with_capacity(1024);
    
    let (north, south, east, west, up, down) = neighbors;

    // precompute models for the palettes of this chunk and neighboring chunks.
    // Reduces number of CPU cache misses and time spent hashing BlockStates 
    // as indexing a linear data structure is significantly faster. 
    let mut models: [Vec<Option<&BlockModelMinimal>>; 7] = [
        Vec::with_capacity(chunk.palette_len()),
        Vec::with_capacity(north.palette_len()),
        Vec::with_capacity(south.palette_len()),
        Vec::with_capacity(east.palette_len()),
        Vec::with_capacity(west.palette_len()),
        Vec::with_capacity(up.palette_len()),
        Vec::with_capacity(down.palette_len()),
    ];
    setup_model_cache(&chunk, &mut models[0], &model_map);
    setup_model_cache(&north, &mut models[1], &model_map);
    setup_model_cache(&south, &mut models[2], &model_map);
    setup_model_cache(&east, &mut models[3], &model_map);
    setup_model_cache(&west, &mut models[4], &model_map);
    setup_model_cache(&up, &mut models[5], &model_map);
    setup_model_cache(&down, &mut models[6], &model_map);
    let after_model_cache = now.elapsed().as_secs_f64() * 1000.;



    // Figures out cull info for non air blocks.
    for i in 0..ChunkData::BLOCKS_PER_CHUNK {
        let id = chunk.block_at_index(i);

        let block_id = chunk.lookup_palette(id).unwrap();
        if block_id.block.is_air() {
            continue;
        }
        let (x, y, z) = index_to_xyz(i);
        // let culled_sides = 0b00111111;
        cull_info.push((ivec3(x as i32, y as i32, z as i32), &block_id.block, culled_sides(&chunk, x, y, z, neighbors, &models)));
    }
    let after_first_loop = now.elapsed().as_secs_f64() * 1000.;




    // grabs faces for non air blocks that shouldn't be culled
    for (pos, block, cull_info) in cull_info {
        let Some(block_model) = model_map.get(block) else {
            continue;
        };
        for face in block_model.face_iter() {
            if let Some(dir) = face.get_cull_mode() {
                if should_skip(dir, cull_info) {
                    continue;
                }
            }
            faces.push((pos, face));
        }
    }
    let after_second_loop = now.elapsed().as_secs_f64() * 1000.;



    // creates face data and sticks it into vecs
    for (pos, face) in faces {
        let (
            mut face_pos,
            mut face_uv0,
            mut face_normal,
            mut face_index,
            mut face_texture_ids
        ) = face.get_face_data(pos.as_vec3(), indices_offset);

        indices_offset += face_pos.len() as u32;

        positions.append(&mut face_pos);
        uv0s.append(&mut face_uv0);
        normals.append(&mut face_normal);
        indices.append(&mut face_index);
        texture_ids.append(&mut face_texture_ids);
    }
    let after_third_loop = now.elapsed().as_secs_f64() * 1000.;

    
    // creates the chunk mesh
    let ret = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(BlockMaterial::ATTRIBUTE_ARRAY_ID, texture_ids)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv0s)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_indices(Indices::U32(indices));

    let end = now.elapsed().as_secs_f64() * 1000.0;
    if end > 10.0 {
        info!("Took {end} ms to mesh.\nModel cache took {}.First loop took {}, second loop took {}, third loop took {}.", after_model_cache, after_first_loop - after_model_cache, after_second_loop - after_first_loop, after_third_loop - after_second_loop);    
    }

    ret
}

fn should_skip(dir: Direction, cull_info: u8) -> bool {
    match dir {
        Direction::North => cull_info & (0b1) != 0,
        Direction::South => cull_info & (0b1 << 1) != 0,
        Direction::East => cull_info & (0b1 << 2) != 0,
        Direction::West => cull_info & (0b1 << 3) != 0,
        Direction::Up => cull_info & (0b1 << 4) != 0,
        Direction::Down => cull_info & (0b1 << 5) != 0,
    }
}

fn setup_model_cache<'a>(
    chunk: &ChunkData,
    list: &mut Vec<Option<&'a BlockModelMinimal>>,
    model_map: &'a HashMap<BlockState, BlockModelMinimal>
) {
    for entry in 0..chunk.palette_len() {
        let state = &chunk.lookup_palette(entry).unwrap().block;
        let model = model_map.get(&state);
        list.push(model);
    }
}


#[deprecated]
type FaceData = ([[f32; 3]; 4], [[f32; 2]; 4], [[f32; 3]; 4], [u32; 6]);

// outputs vertex specific data for this block and face
#[deprecated]
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

#[deprecated]
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

// TODO: Current bottleneck when it comes to meshing. Speed this up dramatically somehow.
fn culled_sides(
    chunk: &ChunkData,
    x: usize, y: usize, z: usize,
    neighbors: NeighborData,
    model_map: &[Vec<Option<&BlockModelMinimal>>; 7]
) -> u8 {
    let last = ChunkData::CHUNK_SIZE - 1;
    let (north, south, east, west, up, down) = neighbors;

    let (id_north, q_north) = if z == last {
        (north.block_at(x, y, 0), 1)
    } else {
        (chunk.block_at(x, y, z + 1), 0)
    };

    let (id_south, q_south) = if z == 0 {
        (south.block_at(x, y, last), 2)
    } else {
        (chunk.block_at(x, y, z - 1), 0)
    };

    let (id_east, q_east) = if x == last {
        (east.block_at(0, y, z), 3)
    } else {
        (chunk.block_at(x + 1, y, z), 0)
    };

    let (id_west, q_west) = if x == 0 {
        (west.block_at(last, y, z), 4)
    } else {
        (chunk.block_at(x - 1, y, z), 0)
    };

    let (id_up, q_up) = if y == last {
        (up.block_at(x, 0, z), 5)
    } else {
        (chunk.block_at(x, y + 1, z), 0)
    };

    let (id_down, q_down) = if y == 0 {
        (down.block_at(x, last, z), 6)
    } else {
        (chunk.block_at(x, y - 1, z), 0)
    };
    
    let m_north = model_map[q_north][id_north];
    let m_south = model_map[q_south][id_south];
    let m_east = model_map[q_east][id_east];
    let m_west = model_map[q_west][id_west];
    let m_up = model_map[q_up][id_up];
    let m_down = model_map[q_down][id_down];



    let cull_north = match m_north {
        Some(model) => model.is_full(Direction::South),
        None => false,
    } as u8;
    let cull_south = match m_south {
        Some(model) => model.is_full(Direction::North),
        None => false,
    } as u8;
    let cull_east = match m_east {
        Some(model) => model.is_full(Direction::West),
        None => false,
    } as u8;
    let cull_west = match m_west {
        Some(model) => model.is_full(Direction::East),
        None => false,
    } as u8;
    let cull_up = match m_up {
        Some(model) => model.is_full(Direction::Down),
        None => false,
    } as u8;
    let cull_down = match m_down {
        Some(model) => model.is_full(Direction::Up),
        None => false,
    } as u8;
    
    (cull_north) | (cull_south << 1) | (cull_east << 2) | (cull_west << 3) | (cull_up << 4) | (cull_down << 5)
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